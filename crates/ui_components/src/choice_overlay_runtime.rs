//! Internal runtime helpers for choice-family overlays.

use open_gpui::{App, Window};
use open_gpui_ui_core::DismissReason;

use crate::overlay::{OverlayLayerBinding, WindowOverlayRuntime};

pub(crate) fn request_registered_choice_selection(
    window_overlay_runtime: &WindowOverlayRuntime,
    overlay_binding: &OverlayLayerBinding,
    window: &mut Window,
    cx: &mut App,
    transaction: impl FnOnce(&mut Window, &mut App) + 'static,
) {
    window_overlay_runtime
        .request_open_change_with_effect(
            overlay_binding,
            false,
            DismissReason::Selection,
            window,
            cx,
            transaction,
        )
        .expect("choice selection should own its overlay registration");
}
