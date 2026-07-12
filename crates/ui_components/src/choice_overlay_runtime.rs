//! Internal runtime helpers for choice-family overlays.

use open_gpui::{App, Entity, Window};
use open_gpui_ui_core::DismissReason;

use crate::overlay::{OverlayLayerBinding, WindowOverlayRuntime};

pub(crate) trait ChoiceOverlayRuntimeState {
    fn commit_single_value(&mut self, value: String);
}

pub(crate) fn commit_registered_choice_overlay_single_value<T>(
    window_overlay_runtime: &WindowOverlayRuntime,
    overlay_binding: &OverlayLayerBinding,
    runtime: Entity<T>,
    value: String,
    window: &mut Window,
    cx: &mut App,
    after_update: impl FnOnce(&mut Window, &mut App),
) where
    T: ChoiceOverlayRuntimeState + 'static,
{
    window_overlay_runtime
        .request_open_change_with_effect(
            overlay_binding,
            false,
            DismissReason::Selection,
            window,
            cx,
            move |window, cx| {
                runtime.update(cx, |runtime, _| runtime.commit_single_value(value));
                after_update(window, cx);
            },
        )
        .expect("choice selection should own its overlay registration");
}
