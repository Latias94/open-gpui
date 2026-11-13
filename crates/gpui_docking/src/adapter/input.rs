//! Event-time submitted geometry and receiver facts for local surface input.

use dockspace::{
    geometry::LogicalPoint,
    model::SurfaceId,
    runtime::{
        DockspaceHostFrame, HostPointerInput, HostPointerRoute, PointerDeviceKind,
        PointerEventMetadata, PresentedDockReceiver, PresentedDockspaceSurface,
        SurfacePointerCapture, SurfacePointerEvent, SurfacePointerId, SurfacePointerReceiverFacts,
    },
};
use open_gpui::{Bounds, HitboxId, Modifiers, Pixels, Point, Window, WindowId};

use super::HostState;

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
                    None => facts.with_no_delivery(surface),
                };
                facts = match &self.drag {
                    Some(receiver) => facts.with_drag_delivery(receiver),
                    None => facts.with_no_drag_delivery(surface),
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
    let clear = hitboxes
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
        let bounds = mount.regions.get(&source.surface())?.bounds?;
        logical_point(position, bounds)
    });
    if let Some(mount) = host.mounts.get_mut(&owner) {
        mount.wake = None;
    }
    host.inputs.push(QueuedPointer {
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
    });
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
