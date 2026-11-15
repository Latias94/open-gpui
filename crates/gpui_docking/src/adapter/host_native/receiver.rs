//! Native receiver answers use the original submitted artifact and committed hit graph.

use std::collections::BTreeMap;

use dockspace::runtime::{
    NativeReceiverAnswer, NativeReceiverPurpose, NativeReceiverQuery, NativeWindowHit,
};
use open_gpui::{App, Pixels, Window, WindowId, point, px};

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
    resolve_in_window(query, mount, |hitbox, point| {
        if let Some(window) = current.filter(|window| window.window_handle().window_id() == owner) {
            hitbox.window_point_target_rank(point, window)
        } else {
            cx.window_point_target_rank(mount.window, hitbox, point)
        }
    })
}

pub(super) fn resolve_in_window(
    query: NativeReceiverQuery,
    mount: &WindowMount,
    target_rank: impl Fn(open_gpui::HitboxId, open_gpui::Point<Pixels>) -> Option<usize>,
) -> NativeReceiverAnswer {
    match query {
        NativeReceiverQuery::WindowHit(query) => {
            for frame in &mount.input_frames {
                let Some((serial, artifact)) = mount
                    .regions
                    .get(&frame.surface)
                    .and_then(|region| region.presented)
                else {
                    continue;
                };
                if serial != frame.serial {
                    continue;
                }
                let point = point(
                    px((query.point().x() / frame.scale) as f32),
                    px((query.point().y() / frame.scale) as f32),
                );
                if !frame.bounds.contains(&point) {
                    continue;
                }
                let exposed = frame
                    .hitboxes
                    .iter()
                    .any(|hitbox| target_rank(*hitbox, point) == Some(0));
                return NativeReceiverAnswer::WindowHit(if exposed {
                    NativeWindowHit::Surface(artifact)
                } else {
                    NativeWindowHit::Blocked
                });
            }
            NativeReceiverAnswer::WindowHit(NativeWindowHit::Unknown)
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
            if let Some((descriptor, _)) = frame
                .receivers
                .iter()
                .find(|(_, hitbox)| target_rank(*hitbox, point) == Some(0))
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
            } else if frame
                .hitboxes
                .iter()
                .any(|hitbox| target_rank(*hitbox, point) == Some(0))
            {
                if matches!(query.purpose(), NativeReceiverPurpose::HoverHit) {
                    // The exposed background clears framework occlusion; the core
                    // resolves dynamic hover targets after preceding pointer edges.
                    NativeReceiverAnswer::ClearHoverLane(query.presented_surface())
                } else {
                    NativeReceiverAnswer::EmptyLane(query.presented_surface())
                }
            } else {
                NativeReceiverAnswer::Blocked(query.presented_surface())
            }
        }
    }
}
