//! A double click adds one semantic reset after its original release.

use dockspace::runtime::{DeferredSurfaceAction, DockspaceReceiverRole};
use open_gpui::Point;

use super::*;
use crate::adapter::SplitterDoubleClick;

pub(super) struct Press {
    surface: SurfaceId,
    visual: DockspaceVisualId,
    position: Point<Pixels>,
    action: DeferredSurfaceAction,
}

pub(super) fn pressed(
    host: &HostState,
    interaction: &InteractionState,
    surface: SurfaceId,
    controls: &[(Rc<Control>, FocusHandle, HitboxId)],
    event: &MouseDownEvent,
    window: &Window,
) {
    let mut pending = interaction.splitter_press.borrow_mut();
    pending.take();
    if event.click_count != 2 || host.ensure_application_admission().is_err() {
        return;
    }
    let key = match host.config.input.splitter_double_click {
        Some(SplitterDoubleClick::Preferred) => ControlAction::Activate,
        Some(SplitterDoubleClick::Equalize) => ControlAction::Equalize,
        None => return,
    };
    let Some((control, receiver)) = controls.iter().rev().find_map(|(control, _, hitbox)| {
        let receiver = control.receiver?;
        (receiver.role() == DockspaceReceiverRole::Splitter
            && hitbox.window_point_target_rank(event.position, window) == Some(0))
        .then_some((control, receiver))
    }) else {
        return;
    };
    let Some(action) = control.slot(key).and_then(|slot| slot.borrow().clone()) else {
        return;
    };
    *pending = Some(Press {
        surface,
        visual: receiver.visual_id(),
        position: event.position,
        action: action.intent,
    });
}

pub(super) fn moved(interaction: &InteractionState, position: Point<Pixels>) {
    // Match GPUI's ordinary click/drag slop. Remember any excursion, including
    // a drag that returns to its starting point before the release.
    if interaction
        .splitter_press
        .borrow()
        .as_ref()
        .is_some_and(|press| (position - press.position).magnitude() > 2.0)
    {
        interaction.splitter_press.borrow_mut().take();
    }
}

pub(super) fn released(
    host: &mut HostState,
    interaction: &InteractionState,
    surface: SurfaceId,
    receivers: &[(DockspaceReceiverDescriptor, HitboxId)],
    event: &MouseUpEvent,
    window: &Window,
    cx: &mut App,
) {
    moved(interaction, event.position);
    let Some(press) = interaction.splitter_press.borrow_mut().take() else {
        return;
    };
    if press.surface != surface
        || event.click_count != 2
        || !receivers.iter().any(|(receiver, hitbox)| {
            receiver.visual_id() == press.visual
                && receiver.role() == DockspaceReceiverRole::Splitter
                && hitbox.window_point_target_rank(event.position, window) == Some(0)
        })
    {
        return;
    }
    input::queue_action(host, press.action, cx);
}
