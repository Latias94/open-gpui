//! Internal runtime helpers for choice-family overlays.

use std::rc::Rc;

use open_gpui::{App, Entity, FocusHandle, Window};
use open_gpui_ui_core::FocusRestoreIntent;

use crate::overlay::{OverlayCloseRuntimeRequest, OverlayLayerHost, set_overlay_open};

pub(crate) type ChoiceOverlayOpenChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

pub(crate) trait ChoiceOverlayRuntimeState {
    fn open_mut(&mut self) -> &mut bool;
    fn trigger_focus(&self) -> FocusHandle;
    fn commit_single_value(&mut self, value: String);
}

pub(crate) fn close_choice_overlay<T>(
    overlay_host: &OverlayLayerHost,
    runtime: Entity<T>,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<ChoiceOverlayOpenChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) where
    T: ChoiceOverlayRuntimeState + 'static,
{
    let trigger_focus = runtime.read(cx).trigger_focus();
    overlay_host.close_runtime(
        OverlayCloseRuntimeRequest::new(
            runtime,
            &focus_restore,
            trigger_focus,
            on_open_change.as_deref(),
        ),
        window,
        cx,
        |runtime| {
            set_overlay_open(runtime.open_mut(), false);
        },
    );
}

pub(crate) fn commit_choice_overlay_single_value<T>(
    overlay_host: &OverlayLayerHost,
    runtime: Entity<T>,
    focus_restore: FocusRestoreIntent,
    on_open_change: Option<ChoiceOverlayOpenChangeHandler>,
    value: String,
    window: &mut Window,
    cx: &mut App,
    after_update: impl FnOnce(&mut Window, &mut App),
) where
    T: ChoiceOverlayRuntimeState + 'static,
{
    let trigger_focus = runtime.read(cx).trigger_focus();
    overlay_host.close_runtime_with_after_update(
        OverlayCloseRuntimeRequest::new(
            runtime,
            &focus_restore,
            trigger_focus,
            on_open_change.as_deref(),
        ),
        window,
        cx,
        move |runtime| {
            runtime.commit_single_value(value);
            set_overlay_open(runtime.open_mut(), false);
        },
        after_update,
    );
}
