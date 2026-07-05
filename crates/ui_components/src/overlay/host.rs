//! Shared GPUI overlay host facade.

use open_gpui::{AnyElement, App, IntoElement, Pixels, Point, Window};
use open_gpui_ui_core::OverlayLayerPolicy;

use super::OverlayResolvedState;
use super::adapter::{
    GpuiOverlayState, gpui_full_window_overlay_layer, gpui_overlay_state,
    gpui_positioned_overlay_layer, gpui_relative_overlay_layer,
};
use super::placement::GpuiOverlayPlacement;
use super::runtime::{
    OverlayCloseRuntimeRequest, OverlayOpenChange, OverlayOpenRuntimeRequest,
    apply_overlay_open_change, apply_overlay_open_change_with_after_update, close_overlay_runtime,
    close_overlay_runtime_with_after_update, consume_overlay_event, escape_open_change,
    outside_press_open_change,
};

/// Adapter-owned host for GPUI overlay layer lifecycle decisions.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OverlayLayerHost {
    adapter: GpuiOverlayState,
}

impl OverlayLayerHost {
    /// Resolves a host from renderer-neutral overlay state.
    pub(crate) fn resolve(overlay: &OverlayResolvedState) -> Self {
        Self {
            adapter: gpui_overlay_state(overlay),
        }
    }

    /// Returns the GPUI adapter state.
    pub(crate) const fn adapter(&self) -> &GpuiOverlayState {
        &self.adapter
    }

    /// Returns the shared overlay policy.
    pub(crate) const fn policy(&self) -> &OverlayLayerPolicy {
        self.adapter.policy()
    }

    /// Resolves the Escape-key open-change request for the hosted layer.
    pub(crate) const fn escape_open_change(&self) -> Option<OverlayOpenChange> {
        escape_open_change(self.policy())
    }

    /// Resolves the outside-press open-change request for the hosted layer.
    pub(crate) const fn outside_press_open_change(&self) -> Option<OverlayOpenChange> {
        outside_press_open_change(self.policy())
    }

    /// Consumes a GPUI event according to the hosted overlay lifecycle.
    pub(crate) fn consume_event(&self, window: &mut Window, cx: &mut App) {
        consume_overlay_event(window, cx);
    }

    /// Applies an open-state transition through the shared overlay lifecycle.
    pub(crate) fn apply_open_change<T: 'static>(
        &self,
        request: OverlayOpenRuntimeRequest<'_, T>,
        window: &mut Window,
        cx: &mut App,
        update_runtime: impl FnOnce(&mut T),
    ) {
        apply_overlay_open_change(request, window, cx, update_runtime);
    }

    /// Applies an open-state transition and then runs a post-update hook.
    pub(crate) fn apply_open_change_with_after_update<T: 'static>(
        &self,
        request: OverlayOpenRuntimeRequest<'_, T>,
        window: &mut Window,
        cx: &mut App,
        update_runtime: impl FnOnce(&mut T),
        after_update: impl FnOnce(&mut Window, &mut App),
    ) {
        apply_overlay_open_change_with_after_update(
            request,
            window,
            cx,
            update_runtime,
            after_update,
        );
    }

    /// Closes a runtime through the shared callback/focus tail.
    pub(crate) fn close_runtime<T: 'static>(
        &self,
        request: OverlayCloseRuntimeRequest<'_, T>,
        window: &mut Window,
        cx: &mut App,
        close_runtime: impl FnOnce(&mut T),
    ) {
        close_overlay_runtime(request, window, cx, close_runtime);
    }

    /// Closes a runtime, runs a post-update hook, and applies the callback/focus tail.
    pub(crate) fn close_runtime_with_after_update<T: 'static>(
        &self,
        request: OverlayCloseRuntimeRequest<'_, T>,
        window: &mut Window,
        cx: &mut App,
        close_runtime: impl FnOnce(&mut T),
        after_update: impl FnOnce(&mut Window, &mut App),
    ) {
        close_overlay_runtime_with_after_update(request, window, cx, close_runtime, after_update);
    }

    /// Builds a deferred GPUI anchored overlay without forcing a window position.
    pub(crate) fn relative_layer(
        &self,
        placement: &GpuiOverlayPlacement,
        child: impl IntoElement,
    ) -> AnyElement {
        gpui_relative_overlay_layer(&self.adapter, placement, child)
    }

    /// Builds a deferred GPUI anchored overlay at the resolved window position.
    pub(crate) fn positioned_layer(
        &self,
        placement: &GpuiOverlayPlacement,
        fallback_position: Point<Pixels>,
        child: impl IntoElement,
    ) -> AnyElement {
        gpui_positioned_overlay_layer(&self.adapter, placement, fallback_position, child)
    }

    /// Builds a deferred GPUI full-window overlay layer.
    pub(crate) fn full_window_layer(&self, child: impl IntoElement) -> AnyElement {
        gpui_full_window_overlay_layer(&self.adapter, child)
    }
}
