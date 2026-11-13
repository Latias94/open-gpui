//! Stationary gesture work remains tied to its submitted physical frame.

use super::{SharedHost, input};
use dockspace::{model::SurfaceId, runtime::DockspacePointerContinuation};
use open_gpui::{
    App, Bounds, HitboxId, Pixels, PlatformHoveredWindow, PointerCaptureHandle, Task, Window,
};
use std::rc::Rc;

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

pub(super) struct Wake {
    request: DockspacePointerContinuation,
    serial: u64,
    _task: Task<()>,
}

pub(super) fn schedule(shared: &SharedHost, window: &Window, cx: &App) {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    if host.native.is_some() {
        return;
    }
    let ready = host.session.pointer_continuation().and_then(|request| {
        let mount = host.mounts.get(&owner)?;
        let frame = mount
            .input_frames
            .iter()
            .find(|frame| frame.surface == request.surface())?;
        (mount.pending.is_none()
            && host.inputs.is_empty()
            && frame.serial == mount.serial
            && input::is_presented(&host, owner, frame.surface, frame.serial))
        .then_some((request, frame.serial))
    });
    let Some((request, serial)) = ready else {
        if let Some(mount) = host.mounts.get_mut(&owner) {
            mount.wake = None;
        }
        return;
    };
    if host
        .mounts
        .get(&owner)
        .and_then(|mount| mount.wake.as_ref())
        .is_some_and(|wake| wake.request == request && wake.serial == serial)
    {
        return;
    }
    let timer = host
        .clock
        .timer(request.deadline().saturating_sub(host.observed_at()));
    let weak = Rc::downgrade(shared);
    let task = window.spawn(cx, async move |cx| {
        timer.await;
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let _ = cx.update(|window, cx| wake(&shared, request, serial, window, cx));
    });
    if let Some(mount) = host.mounts.get_mut(&owner) {
        mount.wake = Some(Wake {
            request,
            serial,
            _task: task,
        });
    }
}

fn wake(
    shared: &SharedHost,
    request: DockspacePointerContinuation,
    serial: u64,
    window: &mut Window,
    cx: &App,
) {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    let Some(mount) = host.mounts.get_mut(&owner) else {
        return;
    };
    if !mount
        .wake
        .as_ref()
        .is_some_and(|wake| wake.request == request && wake.serial == serial)
    {
        return;
    }
    mount.wake = None;
    let Some(frame) = mount
        .input_frames
        .iter()
        .find(|frame| frame.surface == request.surface())
        .cloned()
    else {
        return;
    };
    let pending = mount.pending.is_some();
    let current = mount.serial;
    let hovered = match cx.hovered_window() {
        PlatformHoveredWindow::Window(handle) => handle == window.window_handle(),
        PlatformHoveredWindow::NoWindow => false,
        PlatformHoveredWindow::Unavailable => window.is_window_hovered(),
    };
    let position = window.mouse_position();
    if host.session.pointer_continuation() != Some(request)
        || host.observed_at() < request.deadline()
        || pending
        || !host.inputs.is_empty()
        || serial != current
        || !input::is_presented(&host, owner, frame.surface, serial)
        || !window.is_window_active()
        || !hovered
        || !frame.capture.is_captured(window)
        || input::logical_point(position, frame.bounds) != Some(request.position())
        || !frame
            .hitboxes
            .iter()
            .any(|hitbox| hitbox.window_point_target_rank(position, window) == Some(0))
    {
        return;
    }
    if input::queue(
        &mut host,
        frame.surface,
        serial,
        request.event(),
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
