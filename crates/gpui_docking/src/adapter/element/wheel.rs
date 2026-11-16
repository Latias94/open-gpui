//! Chrome scrolling follows real GPUI hits and retains phased gesture ownership.

use dockspace::{
    model::WorkspaceVersion,
    runtime::{SurfacePaintProgramOperation as Operation, SurfacePaintProgramStep as Step},
};
use open_gpui::{ScrollEvent, ScrollSource, TouchPhase};

use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum ScrollTarget {
    Strip(DockspaceReceiverDescriptor),
    Menu(DockspaceReceiverDescriptor),
}

impl ScrollTarget {
    fn receiver(self) -> DockspaceReceiverDescriptor {
        match self {
            Self::Strip(receiver) | Self::Menu(receiver) => receiver,
        }
    }

    fn delta(self, event: &ScrollEvent, line_height: Pixels) -> f64 {
        let delta = event.delta.pixel_delta(line_height);
        let amount = match self {
            Self::Strip(_) if delta.x != px(0.0) => delta.x,
            Self::Strip(_) | Self::Menu(_) => delta.y,
        };
        -f64::from(f32::from(amount))
    }
}

/// `None` inside an observed gesture means content or an unknown overlay owns
/// it. Keep that owner through Ended and precise tail events until the next
/// Started edge. A coarse wheel notch starts its own unphased interaction.
/// Without an observed gesture, precise tail ownership remains unknown.
/// Ownership cannot cross the starting workspace's replacement epoch.
#[derive(Clone, Copy)]
pub(super) struct WheelGesture {
    owner: Option<(SurfaceId, DockspaceVisualId)>,
    source: ScrollSource,
    version: WorkspaceVersion,
}

fn accepts_modifiers(event: &ScrollEvent) -> bool {
    !event.modifiers.control
        && !event.modifiers.platform
        && !event.modifiers.alt
        && !event.modifiers.function
}

fn accepts_scroll(host: &HostState) -> bool {
    host.ensure_application_admission().is_ok()
        && !host.has_unaccepted_update()
        && host.inputs.is_empty()
        && !host.session.has_active_gesture()
        && !host.session.has_pending_native_input()
        && host
            .native
            .as_ref()
            .is_none_or(|native| native.transport.is_empty())
}

/// Resolve Started against the whole committed mount, including starts outside
/// every region. Each region listener computes the same owner, so a later
/// listener cannot replace another region's chrome claim with a local miss.
fn gesture_owner(
    host: &HostState,
    event: &ScrollEvent,
    window: &Window,
) -> Option<(SurfaceId, DockspaceVisualId)> {
    if !accepts_modifiers(event) || !accepts_scroll(host) {
        return None;
    }
    let owner = window.window_handle().window_id();
    let mount = host.mounts.get(&owner).filter(|mount| mount.attached)?;
    mount.input_frames.iter().find_map(|frame| {
        if frame.serial != mount.serial
            || !frame.bounds.contains(&event.position)
            || !input::is_presented(host, owner, frame.surface, frame.serial)
            || !frame
                .hitboxes
                .iter()
                .any(|hitbox| hitbox.window_point_target_rank(event.position, window) == Some(0))
        {
            return None;
        }
        let point = input::logical_point(event.position, frame.bounds)?;
        let packet = mount.regions.get(&frame.surface)?.packet.as_ref()?;
        packet.draws.iter().rev().find_map(|draw| match draw {
            Draw::Scroll(target) if target.receiver().bounds().contains(point) => {
                Some((frame.surface, target.receiver().visual_id()))
            }
            _ => None,
        })
    })
}

pub(super) fn listen(
    shared: &SharedHost,
    interaction: &Rc<InteractionState>,
    region: &PreparedRegion,
    window: &mut Window,
) {
    let shared = shared.clone();
    let interaction = interaction.clone();
    let bounds = region.bounds;
    let surface = region.surface;
    let serial = region.serial;
    let hitboxes = region.hitboxes.clone();
    let targets = region.scrolls.clone();
    window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
        if phase.capture() {
            match event.touch_phase {
                TouchPhase::Cancelled => interaction.wheel_gesture.set(Some(WheelGesture {
                    owner: None,
                    source: event.source,
                    version: shared.borrow().session.version(),
                })),
                TouchPhase::Started if event.delta.precise() => {
                    let host = shared.borrow();
                    interaction.wheel_gesture.set(Some(WheelGesture {
                        owner: gesture_owner(&host, event, window),
                        source: event.source,
                        version: host.session.version(),
                    }));
                }
                TouchPhase::Ended => {}
                _ if !event.delta.precise() => interaction.wheel_gesture.set(None),
                _ => {}
            }
            return;
        }
        if !phase.bubble()
            || event.touch_phase == TouchPhase::Cancelled
            || !accepts_modifiers(event)
        {
            return;
        }
        let local = input::logical_point(event.position, bounds);
        let target = local.and_then(|point| {
            targets.iter().rev().copied().find(|target| {
                target.receiver().bounds().contains(point)
                    && hitboxes.iter().any(|hitbox| {
                        hitbox.window_point_target_rank(event.position, window) == Some(0)
                    })
            })
        });
        let Some((target, point)) = target.zip(local) else {
            return;
        };
        let mut host = shared.borrow_mut();
        if (event.delta.precise() || event.touch_phase == TouchPhase::Ended)
            && !interaction.wheel_gesture.get().is_some_and(|gesture| {
                gesture.version.is_same_epoch(host.session.version())
                    && gesture.source == event.source
                    && gesture.owner == Some((surface, target.receiver().visual_id()))
            })
        {
            return;
        }
        let owner = window.window_handle().window_id();
        if !input::is_presented(&host, owner, surface, serial) || !accepts_scroll(&host) {
            return;
        }
        let delta = target.delta(event, host.config.text_size * 1.5);
        if !delta.is_finite() || delta == 0.0 {
            return;
        }
        let result = (|| -> Result<_> {
            let HostState {
                session,
                mounts,
                native,
                ..
            } = &mut *host;
            let mut frame =
                host_native::begin_frame(session, mounts, native.as_ref(), Some(window), cx)?;
            let Some(plan) = frame.paint_plan(surface)? else {
                return Ok(None);
            };
            let mut owned = false;
            let mut action = None;
            plan.visit_surface_program(|step| {
                match (target, step) {
                    (
                        ScrollTarget::Strip(receiver),
                        Step::Operation {
                            operation: Operation::TabBar(bar),
                            paint,
                            ..
                        },
                    ) if !paint.is_source_gap()
                        && plan
                            .receiver_for_tab_strip_scroll(bar)
                            .is_some_and(|current| {
                                current.visual_id() == receiver.visual_id()
                                    && current.bounds() == receiver.bounds()
                            }) =>
                    {
                        owned = true;
                        action = plan.prepare_tab_strip_scroll_at(bar, point, delta);
                    }
                    (ScrollTarget::Menu(receiver), Step::PopupMenu { menu, paint })
                        if !paint.is_source_gap()
                            && plan.receiver_for_tab_list_menu_scroll(menu).is_some_and(
                                |current| {
                                    current.visual_id() == receiver.visual_id()
                                        && current.bounds() == receiver.bounds()
                                },
                            ) =>
                    {
                        owned = true;
                        action = plan.prepare_tab_list_menu_scroll_at(menu, point, delta);
                    }
                    _ => {}
                }
                Ok::<_, dockspace::runtime::DockspaceRuntimeError>(())
            })?;
            if !owned {
                return Ok(None);
            }
            let Some(action) = action else {
                // A clamped chrome scroller still owns this event. Do not let
                // its tail start scrolling an underlying pane at the limit.
                return Ok(Some(None));
            };
            frame.submit_surface_action(action)?;
            Ok(Some(Some(frame.commit()?)))
        })();
        match result {
            Ok(Some(report)) => {
                if let Some(report) = report {
                    host.apply_report(report);
                    window.refresh();
                }
                drop(host);
                host_native::schedule(&shared, cx);
                cx.stop_propagation();
            }
            Ok(None) => {}
            Err(error) => host.fail(error.as_ref()),
        }
    });
}
