use crate::DockSpaceId;
use open_gpui::{AnyWindowHandle, App, Pixels, Point, Window, WindowId};

/// Result of resolving a screen point into a registered dock viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportHit {
    /// Logical dock space that contains the point.
    pub space: DockSpaceId,
    /// Point relative to the dock host bounds.
    pub host_position: Point<Pixels>,
}

/// A viewport hit with the runtime window that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct DockViewportHitCandidate {
    /// Logical dock space that contains the point.
    pub space: DockSpaceId,
    /// GPUI window currently rendering the logical dock space.
    pub window: AnyWindowHandle,
    /// Point relative to the dock host bounds.
    pub host_position: Point<Pixels>,
}

impl DockViewportHitCandidate {
    pub(crate) fn into_hit(self) -> DockViewportHit {
        DockViewportHit {
            space: self.space,
            host_position: self.host_position,
        }
    }
}

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

pub(crate) fn resolve_viewport_target(
    hits: Vec<DockViewportHitCandidate>,
    context: &DockViewportTargetContext,
) -> Option<DockViewportHitCandidate> {
    hits.into_iter()
        .enumerate()
        .min_by_key(|(index, hit)| {
            let window_id = hit.window.window_id();
            (
                context
                    .hovered_window
                    .map(|hovered| usize::from(hovered != window_id))
                    .unwrap_or(1),
                context
                    .active_window
                    .map(|active| usize::from(active != window_id))
                    .unwrap_or(1),
                context
                    .window_stack
                    .iter()
                    .position(|stacked| *stacked == window_id)
                    .unwrap_or(usize::MAX),
                *index,
            )
        })
        .map(|(_, hit)| hit)
}
