//! Event-time submitted geometry and receiver facts for local surface input.

use std::{cell::Cell, collections::BTreeMap, rc::Rc};

use dockspace::{
    geometry::LogicalPoint,
    model::{DockspaceItemView, ItemId, NodeKey, RootId, SurfaceId, WorkspaceVersion},
    runtime::{
        DeferredSurfaceAction, DockspaceHostFrame, HostPointerInput, HostPointerRoute,
        PointerDeviceKind, PointerEventMetadata, PresentedDockReceiver, PresentedDockspaceSurface,
        SurfacePointerButton, SurfacePointerCancelReason, SurfacePointerCapture,
        SurfacePointerEvent, SurfacePointerId, SurfacePointerReceiverFacts,
    },
};
use open_gpui::{
    Bounds, ElementGeometry, HitboxId, Modifiers, Pixels, Point, PointerCaptureHandle, Window,
    WindowId,
};

use super::{HostState, native::CallbackCustody};

#[derive(Clone, Copy, PartialEq)]
pub(super) struct RegionGeometry {
    pub(super) element: ElementGeometry,
    pub(super) clip: Bounds<Pixels>,
    pub(super) device_scale: f64,
}

impl RegionGeometry {
    pub(super) fn new(bounds: Bounds<Pixels>, window: &Window) -> anyhow::Result<Self> {
        let element = window.try_element_geometry(bounds)?;
        Ok(Self {
            element,
            clip: element.displayed_bounds().intersect(&window.clip_bounds()),
            device_scale: f64::from(window.scale_factor()),
        })
    }
}

#[derive(Clone)]
pub(super) struct InputFrame {
    pub(super) surface: SurfaceId,
    pub(super) serial: u64,
    pub(super) version: WorkspaceVersion,
    pub(super) geometry: ElementGeometry,
    pub(super) hitboxes: Vec<HitboxId>,
    pub(super) content_hitboxes: Vec<HitboxId>,
    pub(super) panes: BTreeMap<ItemId, PanePresentation>,
    pub(super) capture: PointerCaptureHandle,
    pub(super) receivers: Vec<(dockspace::runtime::DockspaceReceiverDescriptor, HitboxId)>,
    pub(super) scale: f64,
    pub(super) native_binding: Option<dockspace::runtime::NativeSurfaceBinding>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PanePlacement {
    root: RootId,
    tabs: Option<NodeKey>,
}

impl PanePlacement {
    pub(super) fn of(pane: DockspaceItemView<'_>) -> Self {
        Self {
            root: pane.root(),
            tabs: pane.tabs().map(|tabs| tabs.key()),
        }
    }
}

#[derive(Clone)]
pub(super) struct PanePresentation {
    pub(super) placement: PanePlacement,
    pub(super) geometry: open_gpui::HitTestSnapshot,
}

pub(super) struct PrimaryPress {
    surface: SurfaceId,
    source: PrimaryPressSource,
}

enum PrimaryPressSource {
    Local {
        committed: Rc<Cell<bool>>,
        captured: bool,
    },
    Native(CallbackCustody),
}

impl PrimaryPress {
    pub(super) fn captured_local_surface(&self) -> Option<SurfaceId> {
        matches!(
            self.source,
            PrimaryPressSource::Local { captured: true, .. }
        )
        .then_some(self.surface)
    }
}

pub(super) fn finish_content_press(
    host: &mut HostState,
    event: SurfacePointerEvent,
    modifiers: Modifiers,
    window: &mut Window,
) -> bool {
    let owner = window.window_handle().window_id();
    let Some(mount) = host.mounts.get_mut(&owner) else {
        return false;
    };
    let Some(press) = mount.primary_press.take_if(|press| {
        matches!(event, SurfacePointerEvent::StreamCancelled(_))
            || !matches!(
                press.source,
                PrimaryPressSource::Local { captured: true, .. }
            )
    }) else {
        return false;
    };
    mount.wake = None;
    if let Some(region) = mount.regions.get_mut(&press.surface) {
        region.dirty = true;
    }
    let stream_live = mount.local_pointer_live.clone();
    let metadata = PointerEventMetadata::new(PointerDeviceKind::Mouse, host.observed_at())
        .with_docking_enabled(host.config.input.docking_enabled(modifiers));
    // The press was observed for bookkeeping and pane activation. Its terminal
    // does not need a new hit test, even when the original region was resized.
    match press.source {
        PrimaryPressSource::Local { committed, .. } => {
            host.inputs.push(QueuedInput::unrouted_terminal(
                owner,
                event,
                Some(committed),
                stream_live,
                metadata,
            ));
        }
        PrimaryPressSource::Native(custody) => {
            super::host_native::queue_content_terminal(host, owner, &custody, event, metadata);
        }
    }
    window.refresh();
    true
}

pub(super) fn retire_local_pointer(
    host: &mut HostState,
    owner: WindowId,
    stream_live: Rc<Cell<bool>>,
) {
    host.inputs.push(QueuedInput::unrouted_terminal(
        owner,
        SurfacePointerEvent::StreamCancelled(SurfacePointerCancelReason::BindingRetired),
        None,
        stream_live,
        PointerEventMetadata::new(PointerDeviceKind::Mouse, host.observed_at()),
    ));
}

/// Semantic pointer consequences share the original input's rollback custody.
#[derive(Clone)]
pub(super) enum QueuedInput {
    Pointer(QueuedPointer),
    Action {
        owner: WindowId,
        action: DeferredSurfaceAction,
    },
}

impl QueuedInput {
    fn unrouted_terminal(
        owner: WindowId,
        event: SurfacePointerEvent,
        press_commit: Option<Rc<Cell<bool>>>,
        stream_live: Rc<Cell<bool>>,
        metadata: PointerEventMetadata,
    ) -> Self {
        Self::Pointer(QueuedPointer {
            owner,
            event,
            manipulation: HostPointerRoute::Unknown,
            hover: HostPointerRoute::Unknown,
            surface: None,
            delivery: None,
            drag: None,
            clear: false,
            delivery_clear: false,
            capture: SurfacePointerCapture::None,
            metadata,
            press_commit,
            stream_live,
        })
    }

    pub(super) fn owner(&self) -> WindowId {
        match self {
            Self::Pointer(pointer) => pointer.owner,
            Self::Action { owner, .. } => *owner,
        }
    }

    pub(super) fn is_required_pointer_terminal(&self) -> bool {
        matches!(self, Self::Pointer(pointer)
            if pointer.event
                == SurfacePointerEvent::StreamCancelled(SurfacePointerCancelReason::BindingRetired)
                || (matches!(pointer.event, SurfacePointerEvent::StreamCancelled(_))
                    && pointer.stream_live.get())
                || (is_primary_terminal(pointer.event)
                    && pointer.press_commit.as_ref().is_some_and(|committed| committed.get())))
    }

    pub(super) fn mark_committed(&self) {
        if let Self::Pointer(pointer) = self {
            if is_primary_press(pointer.event)
                && let Some(committed) = &pointer.press_commit
            {
                committed.set(true);
            }
            pointer.stream_live.set(!matches!(
                pointer.event,
                SurfacePointerEvent::StreamCancelled(_)
            ));
        }
    }

    pub(super) fn submit(&self, frame: &mut DockspaceHostFrame<'_>) -> anyhow::Result<()> {
        match self {
            Self::Pointer(pointer) => pointer.submit(frame),
            Self::Action { action, .. } => {
                frame.submit_deferred_surface_action(action.clone())?;
                Ok(())
            }
        }
    }
}

pub(super) fn queue_action(
    host: &mut HostState,
    owner: WindowId,
    action: DeferredSurfaceAction,
    cx: &mut open_gpui::App,
) {
    if let Some(native) = &host.native {
        native.transport.surface_action(action, cx);
    } else {
        host.inputs.push(QueuedInput::Action { owner, action });
    }
}

#[derive(Clone)]
pub(super) struct QueuedPointer {
    owner: WindowId,
    event: SurfacePointerEvent,
    manipulation: HostPointerRoute,
    hover: HostPointerRoute,
    surface: Option<PresentedDockspaceSurface>,
    delivery: Option<PresentedDockReceiver>,
    drag: Option<PresentedDockReceiver>,
    clear: bool,
    delivery_clear: bool,
    capture: SurfacePointerCapture,
    metadata: PointerEventMetadata,
    press_commit: Option<Rc<Cell<bool>>>,
    stream_live: Rc<Cell<bool>>,
}

fn is_primary_press(event: SurfacePointerEvent) -> bool {
    event == SurfacePointerEvent::ButtonPressed(SurfacePointerButton::Primary)
}

fn is_primary_terminal(event: SurfacePointerEvent) -> bool {
    matches!(
        event,
        SurfacePointerEvent::StreamCancelled(_)
            | SurfacePointerEvent::ButtonReleased(SurfacePointerButton::Primary)
    )
}

pub(super) fn local_pointer(owner: WindowId) -> SurfacePointerId {
    // Each physical window is an independent local pointer provider. Reusing
    // one stream number lets a retired window's terminal edge terminate a
    // newer gesture owned by a sibling window.
    SurfacePointerId::new(owner.as_u64())
}

impl QueuedPointer {
    pub(super) fn submit(&self, frame: &mut DockspaceHostFrame<'_>) -> anyhow::Result<()> {
        let mut facts = SurfacePointerReceiverFacts::unknown();
        if let Some(surface) = &self.surface {
            if self.delivery_clear {
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
            local_pointer(self.owner),
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
    geometry: ElementGeometry,
    hitboxes: &[HitboxId],
    content_hitboxes: &[HitboxId],
    captured: bool,
    modifiers: Modifiers,
    window: &Window,
) -> bool {
    let target = window.frontmost_hitbox_at(position);
    let chrome_clear = target.is_some_and(|id| hitboxes.contains(&id));
    let content_clear = target.is_some_and(|id| content_hitboxes.contains(&id));
    let owner = window.window_handle().window_id();
    if host.native.is_some() {
        let queued = super::host_native::queue_pointer(
            host,
            surface_id,
            serial,
            event,
            position,
            geometry,
            chrome_clear || content_clear,
            captured,
            modifiers,
            window,
        );
        if queued
            && !captured
            && !chrome_clear
            && content_clear
            && is_primary_press(event)
            && let Some(custody) = host
                .native
                .as_ref()
                .and_then(|native| native.callbacks.get(&owner))
                .map(|callbacks| callbacks.callback_custody())
            && let Some(mount) = host.mounts.get_mut(&owner)
        {
            mount.primary_press = Some(PrimaryPress {
                surface: surface_id,
                source: PrimaryPressSource::Native(custody),
            });
        }
        return queued;
    }
    let chrome_clear = window.is_mouse_in_window() && chrome_clear;
    let clear = chrome_clear || (window.is_mouse_in_window() && content_clear);
    let terminal = is_primary_terminal(event);
    if !captured && !clear && !terminal {
        return false;
    }
    let point = logical_point(position, geometry);
    let displayed = host.mounts.get(&owner).and_then(|mount| {
        mount.input_frames.iter().find(|frame| {
            frame.surface == surface_id && frame.serial == serial && frame.geometry == geometry
        })
    });
    let hit = point
        .filter(|_| displayed.is_some() && is_presented(host, owner, surface_id, serial))
        .and_then(|point| {
            host.session
                .host_pointer_hit_for_device(surface_id, point, PointerDeviceKind::Mouse)
        });
    let content_receiver = target.filter(|_| content_clear).and_then(|target| {
        displayed?
            .receivers
            .iter()
            .find(|(_, id)| *id == target)
            .map(|(receiver, _)| *receiver)
    });
    let delivery_clear = chrome_clear
        || content_receiver.is_some_and(|receiver| {
            hit.and_then(|hit| hit.delivery())
                .is_some_and(|delivery| delivery.descriptor() == receiver)
        });
    if hit.is_none() && !captured && !terminal {
        return false;
    }
    let surface = hit.map(|hit| hit.surface());
    let source = captured
        .then(|| {
            let pressed = host
                .mounts
                .get(&owner)
                .and_then(|mount| mount.primary_press.as_ref()?.captured_local_surface());
            host.session
                .host_pointer_manipulation_surface(local_pointer(owner))
                .filter(|source| pressed.is_none_or(|pressed| source.surface() == pressed))
                .or_else(|| host.session.presented_surface(pressed?))
        })
        .flatten()
        .or(surface)
        .or_else(|| {
            terminal
                .then(|| host.session.presented_surface(surface_id))
                .flatten()
        });
    let source_point = source.and_then(|source| {
        let mount = host.mounts.get(&owner)?;
        let geometry = mount
            .input_frames
            .iter()
            .find(|frame| frame.surface == source.surface())?
            .geometry;
        logical_point(position, geometry)
    });
    let Some(mount) = host.mounts.get_mut(&owner) else {
        return false;
    };
    let press_commit = if is_primary_press(event) {
        if mount.primary_press.is_some() {
            return false;
        }
        let committed = Rc::new(Cell::new(false));
        mount.primary_press = Some(PrimaryPress {
            surface: surface_id,
            source: PrimaryPressSource::Local {
                committed: committed.clone(),
                captured,
            },
        });
        Some(committed)
    } else if terminal {
        let Some(PrimaryPress {
            source: PrimaryPressSource::Local { committed, .. },
            ..
        }) = mount.primary_press.take()
        else {
            return false;
        };
        // Keep the original release route while making the terminal survive
        // the loss of the window that supplied the press.
        Some(committed)
    } else {
        None
    };
    let stream_live = mount.local_pointer_live.clone();
    mount.wake = None;
    host.inputs.push(QueuedInput::Pointer(QueuedPointer {
        owner,
        event,
        manipulation: source
            .zip(source_point)
            .map_or(HostPointerRoute::Unknown, |(source, point)| {
                HostPointerRoute::new(source, point)
            }),
        hover: surface
            .zip(point)
            .map_or(HostPointerRoute::Unknown, |(surface, point)| {
                HostPointerRoute::new(surface, point)
            }),
        surface,
        delivery: clear.then(|| hit.and_then(|hit| hit.delivery())).flatten(),
        drag: clear
            .then(|| hit.and_then(|hit| hit.drag_delivery()))
            .flatten(),
        clear,
        delivery_clear,
        capture: if captured {
            SurfacePointerCapture::ProviderEndpoint
        } else {
            SurfacePointerCapture::None
        },
        metadata: PointerEventMetadata::new(PointerDeviceKind::Mouse, host.observed_at())
            .with_docking_enabled(host.config.input.docking_enabled(modifiers)),
        press_commit,
        stream_live,
    }));
    if let Some(region) = host
        .mounts
        .get_mut(&owner)
        .and_then(|mount| mount.regions.get_mut(&surface_id))
    {
        region.pointer = point;
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
                        .chain(&frame.content_hitboxes)
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
                .is_some_and(|(_, point)| logical_point(position, frame.geometry) == Some(point))
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
        frame.geometry,
        &frame.hitboxes,
        &frame.content_hitboxes,
        true,
        window.modifiers(),
        window,
    ) {
        window.refresh();
    }
}

pub(super) fn logical_point(
    position: Point<Pixels>,
    geometry: ElementGeometry,
) -> Option<LogicalPoint> {
    let local = geometry.window_to_local_point(position).ok()?;
    LogicalPoint::new(f64::from(f32::from(local.x)), f64::from(f32::from(local.y))).ok()
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
