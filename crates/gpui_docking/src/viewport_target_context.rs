use open_gpui::{AnyWindowHandle, App, Window, WindowId};

/// Platform facts used to arbitrate overlapping viewport hits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DockViewportTargetContext {
    /// Window currently owning the pointer, when known.
    pub hovered_window: Option<WindowId>,
    /// Platform-active window, when known.
    pub active_window: Option<WindowId>,
    /// Front-to-back window stack, when the platform provides it.
    pub window_stack: Vec<WindowId>,
}

impl DockViewportTargetContext {
    /// Creates an empty target context that falls back to deterministic adapter ordering.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a target context from GPUI application-level platform signals.
    pub fn from_app(cx: &App) -> Self {
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

    /// Builds a target context from GPUI application signals and treats this window as hovered.
    ///
    /// This is intended for pointer-event paths that already know the event window. GPUI app-level
    /// signals provide active-window and stack ordering; the current event window supplies the
    /// more specific hovered-window tie breaker.
    pub fn from_window(window: &Window, cx: &App) -> Self {
        Self::from_app(cx).with_hovered_window(window.window_handle())
    }

    /// Adds the hovered window signal.
    pub fn with_hovered_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.hovered_window = Some(window.into().window_id());
        self
    }

    /// Adds the active window signal.
    pub fn with_active_window(mut self, window: impl Into<AnyWindowHandle>) -> Self {
        self.active_window = Some(window.into().window_id());
        self
    }

    /// Adds the front-to-back window stack signal.
    pub fn with_window_stack(
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
