//! Event-time submitted geometry and receiver facts for local surface input.

use dockspace::{
    geometry::LogicalPoint,
    model::SurfaceId,
    runtime::{
        DeferredSurfaceAction, DockspaceHostFrame, HostPointerInput, HostPointerRoute,
        PointerDeviceKind, PointerEventMetadata, PresentedDockReceiver, PresentedDockspaceSurface,
        SurfacePointerCapture, SurfacePointerEvent, SurfacePointerId, SurfacePointerReceiverFacts,
    },
};
use open_gpui::{
    Bounds, HitboxId, Modifiers, Pixels, Point, PointerCaptureHandle, Window, WindowId,
};

use super::HostState;

#[derive(Clone)]
pub(super) struct InputFrame {
    pub(super) surface: SurfaceId,
    pub(super) serial: u64,
    pub(super) bounds: Bounds<Pixels>,
    pub(super) hitboxes: Vec<HitboxId>,
    pub(super) capture: PointerCaptureHandle,
    pub(super) receivers: Vec<(dockspace::runtime::DockspaceReceiverDescriptor, HitboxId)>,
    pub(super) scale: f64,
    pub(super) native_binding: Option<dockspace::runtime::NativeSurfaceBinding>,
}

/// Semantic pointer consequences share the original input's rollback custody.
#[derive(Clone)]
pub(super) enum QueuedInput {
    Pointer(QueuedPointer),
    Action(DeferredSurfaceAction),
}

impl QueuedInput {
    pub(super) fn submit(&self, frame: &mut DockspaceHostFrame<'_>) -> anyhow::Result<()> {
        match self {
            Self::Pointer(pointer) => pointer.submit(frame),
            Self::Action(action) => {
                frame.submit_deferred_surface_action(action.clone())?;
                Ok(())
            }
        }
    }
}

pub(super) fn queue_action(
    host: &mut HostState,
    action: DeferredSurfaceAction,
    cx: &mut open_gpui::App,
) {
    if let Some(native) = &host.native {
        native.transport.surface_action(action, cx);
    } else {
        host.inputs.push(QueuedInput::Action(action));
    }
}

#[derive(Clone, Copy)]
pub(super) struct QueuedPointer {
    event: SurfacePointerEvent,
    manipulation: HostPointerRoute,
    hover: HostPointerRoute,
    surface: Option<PresentedDockspaceSurface>,
    delivery: Option<PresentedDockReceiver>,
    drag: Option<PresentedDockReceiver>,
    clear: bool,
    capture: SurfacePointerCapture,
    metadata: PointerEventMetadata,
}

impl QueuedPointer {
    pub(super) fn submit(&self, frame: &mut DockspaceHostFrame<'_>) -> anyhow::Result<()> {
        let mut facts = SurfacePointerReceiverFacts::unknown();
        if let Some(surface) = &self.surface {
            if self.clear {
                facts = match &self.delivery {
                    Some(receiver) => facts.with_delivery(receiver),
                    None => facts.with_empty_delivery_lane(surface),
                };
                facts = match &self.drag {
                    Some(receiver) => facts.with_drag_delivery(receiver),
                    None => facts.with_empty_drag_delivery_lane(surface),
                };
            } else {
                facts = facts.with_blocked_delivery(surface);
            }
            facts = if self.clear {
                facts.with_clear_hover_lane(surface)
            } else {
                facts.with_blocked_hover(surface)
            };
        }
        frame.submit_host_pointer_batch([HostPointerInput::new(
            SurfacePointerId::new(1),
            self.event,
            self.manipulation,
            self.hover,
            self.capture,
            facts,
        )
        .with_metadata(self.metadata)])?;
        Ok(())
    }
}

pub(super) fn queue(
    host: &mut HostState,
    surface_id: SurfaceId,
    serial: u64,
    event: SurfacePointerEvent,
    position: Point<Pixels>,
    bounds: Bounds<Pixels>,
    hitboxes: &[HitboxId],
    captured: bool,
    modifiers: Modifiers,
    window: &Window,
) -> bool {
    if host.native.is_some() {
        return super::host_native::queue_pointer(
            host, surface_id, serial, event, position, bounds, hitboxes, captured, modifiers,
            window,
        );
    }
    let Some(point) = logical_point(position, bounds) else {
        return false;
    };
    let owner = window.window_handle().window_id();
    let clear = window.is_mouse_in_window()
        && hitboxes
            .iter()
            .any(|hitbox| hitbox.window_point_target_rank(position, window) == Some(0));
    let cancellation = matches!(event, SurfacePointerEvent::StreamCancelled(_));
    if !captured && !clear && !cancellation {
        return false;
    }
    let hit = is_presented(host, owner, surface_id, serial)
        .then(|| {
            host.session
                .host_pointer_hit_for_device(surface_id, point, PointerDeviceKind::Mouse)
        })
        .flatten();
    if hit.is_none() && !captured && !cancellation {
        return false;
    }
    let surface = hit.map(|hit| hit.surface());
    let source = captured
        .then(|| {
            host.session
                .host_pointer_manipulation_surface(SurfacePointerId::new(1))
        })
        .flatten()
        .or(surface);
    let source_point = source.and_then(|source| {
        let mount = host.mounts.get(&owner)?;
        let bounds = mount
            .input_frames
            .iter()
            .find(|frame| frame.surface == source.surface())?
            .bounds;
        logical_point(position, bounds)
    });
    if let Some(mount) = host.mounts.get_mut(&owner) {
        mount.wake = None;
    }
    host.inputs.push(QueuedInput::Pointer(QueuedPointer {
        event,
        manipulation: source
            .zip(source_point)
            .map_or(HostPointerRoute::Unknown, |(source, point)| {
                HostPointerRoute::new(source, point)
            }),
        hover: surface.map_or(HostPointerRoute::Unknown, |surface| {
            HostPointerRoute::new(surface, point)
        }),
        surface,
        delivery: clear.then(|| hit.and_then(|hit| hit.delivery())).flatten(),
        drag: clear
            .then(|| hit.and_then(|hit| hit.drag_delivery()))
            .flatten(),
        clear,
        capture: if captured {
            SurfacePointerCapture::ProviderEndpoint
        } else {
            SurfacePointerCapture::None
        },
        metadata: PointerEventMetadata::new(PointerDeviceKind::Mouse, host.observed_at())
            .with_docking_enabled(host.config.input.docking_enabled(modifiers)),
    }));
    if let Some(region) = host
        .mounts
        .get_mut(&owner)
        .and_then(|mount| mount.regions.get_mut(&surface_id))
    {
        region.pointer = Some(point);
        region.dirty = true;
    }
    true
}

/// A newly installed hit graph can revoke stationary feedback without moving
/// the pointer. Only matching blocked evidence is submitted; removing an overlay
/// cannot revive the old hover deadline or manufacture a fresh pointer move.
pub(super) fn revoke_blocked_hover(shared: &super::SharedHost, window: &mut Window) {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    if host.native.is_some()
        || host.has_unaccepted_update()
        || !host.inputs.is_empty()
        || !host.session.has_active_gesture()
    {
        return;
    }
    let Some(mount) = host.mounts.get(&owner) else {
        return;
    };
    let pending = mount
        .pending
        .as_ref()
        .filter(|pending| pending.serial == mount.serial)
        .and_then(|pending| pending.accepted_input_frames());
    let frames = pending.map_or_else(|| mount.input_frames.clone(), |frames| frames.to_vec());
    let accepted_pending = pending.is_some();
    let position = window.mouse_position();
    let blocked = frames
        .into_iter()
        .filter(|frame| {
            frame.capture.is_captured(window)
                && (accepted_pending || is_presented(&host, owner, frame.surface, frame.serial))
                && (!window.is_mouse_in_window()
                    || !frame
                        .hitboxes
                        .iter()
                        .any(|hitbox| hitbox.window_point_target_rank(position, window) == Some(0)))
        })
        .collect::<Vec<_>>();
    if blocked.is_empty() {
        return;
    }
    let frame = {
        let Ok(probe) = host.session.begin_host_frame() else {
            return;
        };
        blocked.into_iter().find(|frame| {
            probe
                .host_pointer_hover_position(frame.surface)
                .is_some_and(|(_, point)| logical_point(position, frame.bounds) == Some(point))
        })
    };
    let Some(frame) = frame else {
        return;
    };
    // Accepted hit geometry can veto an old hover even while its new pixels are
    // deferred. queue() supplies Unknown target authority until presentation;
    // the retained source route preserves capture. Keep this edge in the same
    // FIFO so a later clear frame cannot erase the observed blocking evidence.
    if queue(
        &mut host,
        frame.surface,
        frame.serial,
        SurfacePointerEvent::Moved,
        position,
        frame.bounds,
        &frame.hitboxes,
        true,
        window.modifiers(),
        window,
    ) {
        window.refresh();
    }
}

pub(super) fn logical_point(
    position: Point<Pixels>,
    bounds: Bounds<Pixels>,
) -> Option<LogicalPoint> {
    LogicalPoint::new(
        f64::from(f32::from(position.x - bounds.origin.x)),
        f64::from(f32::from(position.y - bounds.origin.y)),
    )
    .ok()
}

pub(super) fn is_presented(
    host: &HostState,
    owner: WindowId,
    surface: SurfaceId,
    serial: u64,
) -> bool {
    if host
        .native
        .as_ref()
        .is_some_and(|native| native.is_failed())
    {
        return false;
    }
    host.mounts
        .get(&owner)
        .and_then(|mount| mount.regions.get(&surface))
        .and_then(|region| region.presented)
        .is_some_and(|(presented_serial, artifact)| {
            serial == presented_serial
                && host
                    .session
                    .presented_surface(surface)
                    .is_some_and(|surface| surface.matches_presentation_artifact(artifact))
        })
}
