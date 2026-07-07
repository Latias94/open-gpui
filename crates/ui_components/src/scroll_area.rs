//! Scroll area component.

use crate::geometry::gpui_px_from_ui;
use crate::scroll_surface::{
    ScrollSurfaceRuntime, scroll_surface_handle, should_reset_scroll_surface,
};
use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, ScrollHandle,
    ScrollViewportChangedEvent, Styled, Window, div, point, px,
};
use open_gpui_ui_core::{Sizable, Size, UiPx, ui_px};

type ScrollAreaViewportChangedListener =
    Box<dyn Fn(&ScrollViewportChangedEvent, &mut Window, &mut App) + 'static>;

/// Scroll direction enabled by a [`ScrollArea`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollAreaAxis {
    /// Scroll vertically and clip horizontal overflow.
    #[default]
    Vertical,
    /// Scroll horizontally and clip vertical overflow.
    Horizontal,
    /// Scroll both horizontally and vertically.
    Both,
}

impl ScrollAreaAxis {
    /// Returns the stable axis label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
            Self::Both => "both",
        }
    }

    /// Returns whether horizontal scrolling is enabled.
    pub const fn scrolls_x(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    /// Returns whether vertical scrolling is enabled.
    pub const fn scrolls_y(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

/// Scroll offset reset behavior for a [`ScrollArea`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollResetPolicy {
    /// Keep the current scroll offset across renders.
    #[default]
    Preserve,
    /// Reset to the viewport origin when the reset key changes after initial mount.
    ResetOnKeyChange,
}

impl ScrollResetPolicy {
    /// Returns the stable reset policy label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preserve => "preserve",
            Self::ResetOnKeyChange => "reset-on-key-change",
        }
    }
}

/// Resolved scroll area metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollAreaMetrics {
    scrollbar_width: UiPx,
}

impl ScrollAreaMetrics {
    /// Resolves metrics from the shared foundation size vocabulary.
    pub const fn from_size(size: Size) -> Self {
        Self {
            scrollbar_width: match size {
                Size::XSmall => ui_px(6.0),
                Size::Small => ui_px(8.0),
                Size::Medium => ui_px(10.0),
                Size::Large => ui_px(12.0),
            },
        }
    }

    /// Returns the layout space reserved for the scrollbar.
    pub const fn scrollbar_width(self) -> UiPx {
        self.scrollbar_width
    }
}

/// Resolved scroll area state used by tests, demos, and rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollAreaState {
    viewport_id: String,
    axis: ScrollAreaAxis,
    size: Size,
    reset_policy: ScrollResetPolicy,
    reset_key: Option<String>,
    metrics: ScrollAreaMetrics,
}

impl ScrollAreaState {
    /// Resolves the public state for a scroll area viewport.
    pub fn resolve(
        viewport_id: impl Into<String>,
        axis: ScrollAreaAxis,
        size: Size,
        reset_policy: ScrollResetPolicy,
        reset_key: Option<String>,
    ) -> Self {
        Self {
            viewport_id: viewport_id.into(),
            axis,
            size,
            reset_policy,
            reset_key,
            metrics: ScrollAreaMetrics::from_size(size),
        }
    }

    /// Returns the stable viewport id used by the adapter.
    pub fn viewport_id(&self) -> &str {
        &self.viewport_id
    }

    /// Returns the enabled scroll axis.
    pub const fn axis(&self) -> ScrollAreaAxis {
        self.axis
    }

    /// Returns the foundation size.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the scroll reset policy.
    pub const fn reset_policy(&self) -> ScrollResetPolicy {
        self.reset_policy
    }

    /// Returns the current reset key, when key-based reset is enabled.
    pub fn reset_key(&self) -> Option<&str> {
        self.reset_key.as_deref()
    }

    /// Returns resolved metrics.
    pub const fn metrics(&self) -> ScrollAreaMetrics {
        self.metrics
    }

    /// Returns whether the viewport scrolls horizontally.
    pub const fn scrolls_x(&self) -> bool {
        self.axis.scrolls_x()
    }

    /// Returns whether the viewport scrolls vertically.
    pub const fn scrolls_y(&self) -> bool {
        self.axis.scrolls_y()
    }

    /// Returns whether the adapter should reset because the reset key changed.
    pub fn should_reset_for_key_change(&self, previous_reset_key: Option<&str>) -> bool {
        should_reset_scroll_surface(
            self.reset_policy == ScrollResetPolicy::ResetOnKeyChange,
            previous_reset_key,
            self.reset_key(),
        )
    }
}

/// A concrete GPUI scroll area viewport.
#[derive(IntoElement)]
pub struct ScrollArea {
    id: ElementId,
    viewport_id: String,
    content: AnyElement,
    axis: ScrollAreaAxis,
    size: Size,
    reset_policy: ScrollResetPolicy,
    reset_key: Option<String>,
    scroll_handle: Option<ScrollHandle>,
    viewport_changed_listener: Option<ScrollAreaViewportChangedListener>,
}

impl ScrollArea {
    /// Creates a new scroll area viewport.
    pub fn new(id: impl Into<String>, content: impl IntoElement) -> Self {
        let viewport_id = id.into();

        Self {
            id: viewport_id.clone().into(),
            viewport_id,
            content: content.into_any_element(),
            axis: ScrollAreaAxis::Vertical,
            size: Size::Medium,
            reset_policy: ScrollResetPolicy::Preserve,
            reset_key: None,
            scroll_handle: None,
            viewport_changed_listener: None,
        }
    }

    /// Applies the enabled scroll axis.
    pub fn axis(mut self, axis: ScrollAreaAxis) -> Self {
        self.axis = axis;
        self
    }

    /// Enables vertical scrolling.
    pub fn vertical(self) -> Self {
        self.axis(ScrollAreaAxis::Vertical)
    }

    /// Enables horizontal scrolling.
    pub fn horizontal(self) -> Self {
        self.axis(ScrollAreaAxis::Horizontal)
    }

    /// Enables horizontal and vertical scrolling.
    pub fn both(self) -> Self {
        self.axis(ScrollAreaAxis::Both)
    }

    /// Uses an externally owned GPUI scroll handle.
    ///
    /// This is an adapter-only escape hatch for applications that need direct offset control.
    /// The handle is not stored in [`ScrollAreaState`].
    pub fn scroll_handle(mut self, scroll_handle: &ScrollHandle) -> Self {
        self.scroll_handle = Some(scroll_handle.clone());
        self
    }

    /// Calls `listener` after GPUI commits the final tracked-scroll viewport for a frame.
    pub fn on_scroll_viewport_changed(
        mut self,
        listener: impl Fn(&ScrollViewportChangedEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.viewport_changed_listener = Some(Box::new(listener));
        self
    }

    /// Resets the viewport to its origin when the key changes after initial mount.
    pub fn reset_on_key(mut self, reset_key: impl Into<String>) -> Self {
        self.reset_policy = ScrollResetPolicy::ResetOnKeyChange;
        self.reset_key = Some(reset_key.into());
        self
    }

    /// Preserves the current scroll offset across renders.
    pub fn preserve_scroll(mut self) -> Self {
        self.reset_policy = ScrollResetPolicy::Preserve;
        self.reset_key = None;
        self
    }

    /// Returns the resolved scroll area state.
    pub fn state(&self) -> ScrollAreaState {
        ScrollAreaState::resolve(
            self.viewport_id.clone(),
            self.axis,
            self.size,
            self.reset_policy,
            self.reset_key.clone(),
        )
    }
}

impl Sizable for ScrollArea {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state();
        let id = self.id;
        let runtime_id: ElementId = (id.clone(), "runtime").into();
        let current_reset_key = state.reset_key().map(str::to_owned);
        let runtime = window.use_keyed_state(runtime_id, cx, {
            let current_reset_key = current_reset_key.clone();
            |_, _| ScrollSurfaceRuntime::new(current_reset_key)
        });
        let runtime_snapshot = runtime.read(cx).clone();
        let previous_reset_key = runtime_snapshot.reset_key().map(str::to_owned);
        let scroll_handle = scroll_surface_handle(&runtime_snapshot, self.scroll_handle.as_ref());

        if state.should_reset_for_key_change(previous_reset_key.as_deref()) {
            scroll_handle.set_offset(point(px(0.0), px(0.0)));
        }

        if previous_reset_key.as_deref() != current_reset_key.as_deref() {
            runtime.update(cx, |runtime, _| {
                runtime.set_reset_key(current_reset_key);
            });
        }

        div()
            .id(id)
            .debug_selector({
                let viewport_id = state.viewport_id().to_owned();
                move || format!("scroll-area:{viewport_id}")
            })
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .scrollbar_width(gpui_px_from_ui(state.metrics().scrollbar_width()))
            .track_scroll(&scroll_handle)
            .when(state.scrolls_x(), |this| this.overflow_x_scroll())
            .when(state.scrolls_y(), |this| this.overflow_y_scroll())
            .when_some(self.viewport_changed_listener, |this, listener| {
                this.on_scroll_viewport_changed(listener)
            })
            .child(self.content)
    }
}
