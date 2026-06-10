use crate::DockViewportTargetContext;
use open_gpui::{AnyWindowHandle, App, Window, WindowId};

/// Snapshot of platform window signals used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DockViewportPlatformSignals {
    /// Window currently owning the pointer, when known.
    pub(crate) hovered_window: Option<WindowId>,
    /// Platform-active window, when known.
    pub(crate) active_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    pub(crate) window_stack: Vec<WindowId>,
}

impl DockViewportPlatformSignals {
    /// Captures GPUI application-level platform signals.
    pub(crate) fn from_app(cx: &App) -> Self {
        Self {
            hovered_window: None,
            active_window: cx.active_window().map(|window| window.window_id()),
            window_stack: cx
                .window_stack()
                .unwrap_or_default()
                .into_iter()
                .map(|window| window.window_id())
                .collect(),
        }
    }

    /// Captures GPUI platform signals and treats this event window as hovered.
    pub(crate) fn from_window(window: &Window, cx: &App) -> Self {
        Self::from_app(cx).with_hovered_window(window.window_handle())
    }

    /// Adds the hovered window signal.
    pub(crate) fn with_hovered_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.hovered_window = Some(window.into().window_id());
        self
    }

    /// Converts the platform snapshot into the pure resolver context.
    pub(crate) fn target_context(&self) -> DockViewportTargetContext {
        DockViewportTargetContext {
            hovered_window: self.hovered_window,
            active_window: self.active_window,
            window_stack: self.window_stack.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_target_context(target_context: DockViewportTargetContext) -> Self {
        Self {
            hovered_window: target_context.hovered_window,
            active_window: target_context.active_window,
            window_stack: target_context.window_stack,
        }
    }
}

impl From<DockViewportPlatformSignals> for DockViewportTargetContext {
    fn from(signals: DockViewportPlatformSignals) -> Self {
        Self {
            hovered_window: signals.hovered_window,
            active_window: signals.active_window,
            window_stack: signals.window_stack,
        }
    }
}
