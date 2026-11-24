//! Native receiver answers use the original submitted artifact and committed hit graph.

use std::collections::BTreeMap;

use dockspace::geometry::PhysicalRect;
use dockspace::runtime::{
    NativeReceiverAnswer, NativeReceiverPurpose, NativeReceiverQuery, NativeRegionWindowHit,
    NativeWindowHit, resolve_region_window_hit,
};
use open_gpui::{App, HitboxId, Pixels, Window, WindowId, point, px};

use super::{ManagedHost, WindowMount};

pub(super) fn resolve(
    query: NativeReceiverQuery,
    mounts: &BTreeMap<WindowId, WindowMount>,
    native: &ManagedHost,
    current: Option<&Window>,
    cx: &mut App,
) -> NativeReceiverAnswer {
    let owner = match query {
        NativeReceiverQuery::WindowHit(query) => {
            native.callbacks.iter().find_map(|(owner, callbacks)| {
                (callbacks.binding() == query.window()).then_some(*owner)
            })
        }
        NativeReceiverQuery::Surface(query) => mounts.iter().find_map(|(owner, mount)| {
            mount
                .regions
                .get(&query.surface())
                .and_then(|region| region.presented)
                .filter(|(_, artifact)| query.matches_presentation_artifact(*artifact))
                .and_then(|_| native.callbacks.get(owner))
                .map(|_| *owner)
        }),
    };
    let Some(owner) = owner else {
        return NativeReceiverAnswer::Unknown;
    };
    let Some(mount) = mounts.get(&owner) else {
        return NativeReceiverAnswer::Unknown;
    };
    // A sibling's immutable hit graph remains readable while its Window is borrowed.
    resolve_in_window(query, mount, |point| {
        if let Some(window) = current.filter(|window| window.window_handle().window_id() == owner) {
            Some(window.frontmost_hitbox_at(point))
        } else {
            cx.window_frontmost_hitbox_at(mount.window, point)
        }
    })
}

pub(super) fn resolve_in_window(
    query: NativeReceiverQuery,
    mount: &WindowMount,
    frontmost: impl Fn(open_gpui::Point<Pixels>) -> Option<Option<HitboxId>>,
) -> NativeReceiverAnswer {
    match query {
        NativeReceiverQuery::WindowHit(query) => {
            let mut regions = mount
                .input_frames
                .iter()
                .filter_map(|frame| {
                    let (serial, artifact) = mount.regions.get(&frame.surface)?.presented?;
                    (serial == frame.serial).then_some((frame, artifact))
                })
                .peekable();
            let Some((first, _)) = regions.peek() else {
                return NativeReceiverAnswer::WindowHit(NativeWindowHit::Unknown);
            };
            let point = point(
                px((query.point().x() / first.scale) as f32),
                px((query.point().y() / first.scale) as f32),
            );
            let Some(target) = frontmost(point) else {
                return NativeReceiverAnswer::WindowHit(NativeWindowHit::Unknown);
            };
            NativeReceiverAnswer::WindowHit(resolve_region_window_hit(
                query.point(),
                regions.filter_map(|(frame, artifact)| {
                    let bounds = physical_rect(frame.geometry.displayed_bounds(), frame.scale)?;
                    let clip = physical_rect(frame.clip, frame.scale)?;
                    let exposed = target.is_some_and(|target| {
                        frame.hitboxes.contains(&target) || frame.content_hitboxes.contains(&target)
                    });
                    Some(NativeRegionWindowHit::new(artifact, bounds, clip, exposed))
                }),
            ))
        }
        NativeReceiverQuery::Surface(query) => {
            let Some(frame) = mount.input_frames.iter().find(|frame| {
                frame.surface == query.surface()
                    && mount
                        .regions
                        .get(&frame.surface)
                        .and_then(|region| region.presented)
                        .is_some_and(|(serial, artifact)| {
                            serial == frame.serial && query.matches_presentation_artifact(artifact)
                        })
            }) else {
                return NativeReceiverAnswer::Unknown;
            };
            let Some(client) = query.client_position() else {
                return NativeReceiverAnswer::Unknown;
            };
            let point = point(
                px((client.x() / frame.scale) as f32),
                px((client.y() / frame.scale) as f32),
            );
            let Some(target) = frontmost(point) else {
                return NativeReceiverAnswer::Unknown;
            };
            if let Some((descriptor, _)) = frame
                .receivers
                .iter()
                .find(|(_, hitbox)| target == Some(*hitbox))
            {
                if matches!(query.purpose(), NativeReceiverPurpose::HoverHit) {
                    query.clear_hover_lane(descriptor).map_or(
                        NativeReceiverAnswer::Unknown,
                        NativeReceiverAnswer::ClearHoverLane,
                    )
                } else {
                    // A frontmost control blocks delivery lanes it does not implement.
                    query.bind_receiver(descriptor).map_or(
                        NativeReceiverAnswer::Blocked(query.presented_surface()),
                        NativeReceiverAnswer::Dock,
                    )
                }
            } else if target.is_some_and(|target| frame.hitboxes.contains(&target)) {
                if matches!(query.purpose(), NativeReceiverPurpose::HoverHit) {
                    // The exposed background clears framework occlusion; the core
                    // resolves dynamic hover targets after preceding pointer edges.
                    NativeReceiverAnswer::ClearHoverLane(query.presented_surface())
                } else {
                    NativeReceiverAnswer::EmptyLane(query.presented_surface())
                }
            } else if matches!(query.purpose(), NativeReceiverPurpose::HoverHit)
                && target.is_some_and(|target| frame.content_hitboxes.contains(&target))
            {
                NativeReceiverAnswer::ClearHoverLane(query.presented_surface())
            } else {
                NativeReceiverAnswer::Blocked(query.presented_surface())
            }
        }
    }
}

fn physical_rect(bounds: open_gpui::Bounds<Pixels>, scale: f64) -> Option<PhysicalRect> {
    let physical = |pixels: Pixels| f64::from(f32::from(pixels)) * scale;
    PhysicalRect::new(
        physical(bounds.origin.x),
        physical(bounds.origin.y),
        physical(bounds.size.width),
        physical(bounds.size.height),
    )
    .ok()
}
