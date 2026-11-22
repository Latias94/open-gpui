//! One exact GPUI ticket covers every logical region in the physical frame.

use std::{collections::BTreeMap, rc::Rc};

use dockspace::{
    model::SurfaceId,
    runtime::{
        DockspaceUpdate, PaintedSurfaceOutput, SurfacePresentationResult, SurfaceUnavailableReason,
    },
};
use open_gpui::{
    AcceptedFrameFence, App, PlatformFrameDeliveryMode, Window, WindowFrameSubmissionOutcome,
    WindowFrameSubmissionTicket, WindowId, WindowInitialPresentationStatus, WindowVisibility,
};

use super::{
    HostState, SharedHost, continuation,
    element::Packet,
    host_native,
    input::{InputFrame, QueuedInput},
};

pub(super) struct PendingSubmission {
    pub(super) serial: u64,
    pub(super) ticket: WindowFrameSubmissionTicket,
    pub(super) packets: BTreeMap<SurfaceId, Rc<Packet>>,
    pub(super) update: Option<DockspaceUpdate>,
    pub(super) inputs: Vec<QueuedInput>,
    input_revoked: bool,
    outputs: Vec<PaintedSurfaceOutput>,
    accepted_generation: Option<u64>,
    input_frames: Vec<InputFrame>,
}

pub(in crate::adapter) enum PendingPresentation {
    Submission {
        owner: WindowId,
        serial: u64,
        outputs: Vec<PaintedSurfaceOutput>,
        input_frames: Vec<InputFrame>,
        result: SurfacePresentationResult,
    },
    MountRemoved {
        owner: WindowId,
        surfaces: Vec<SurfaceId>,
    },
}

impl PendingSubmission {
    pub(super) fn accepted_input_frames(&self) -> Option<&[InputFrame]> {
        self.accepted_generation
            .map(|_| self.input_frames.as_slice())
    }

    pub(super) fn outputs_for_binding(
        &self,
        binding: dockspace::runtime::NativeSurfaceBinding,
    ) -> bool {
        self.outputs
            .iter()
            .any(|output| output.matches_native_binding(binding))
            || self
                .input_frames
                .iter()
                .any(|frame| frame.native_binding == Some(binding))
    }

    pub(super) fn new(
        serial: u64,
        ticket: WindowFrameSubmissionTicket,
        packets: BTreeMap<SurfaceId, Rc<Packet>>,
        update: DockspaceUpdate,
        inputs: Vec<QueuedInput>,
    ) -> Self {
        Self {
            serial,
            ticket,
            packets,
            update: Some(update),
            inputs,
            input_revoked: false,
            outputs: Vec::new(),
            accepted_generation: None,
            input_frames: Vec::new(),
        }
    }

    fn revoke_inputs_from(&mut self, owner: WindowId) -> bool {
        if self.update.is_none() {
            return false;
        }
        let count = self.inputs.len();
        self.inputs.retain(|input| input.owner() != owner);
        let revoked = self.inputs.len() != count;
        // A remount can reuse the WindowId before the original receipt arrives.
        self.input_revoked |= revoked;
        revoked
    }
}

pub(super) fn abandon_unaccepted(host: &mut HostState, window: &mut Window, cx: &mut App) {
    let owner = window.window_handle().window_id();
    if host
        .mounts
        .get(&owner)
        .and_then(|mount| mount.pending.as_ref())
        .is_some_and(|pending| pending.update.is_some())
    {
        withdraw_pending(host, window, cx);
    }
}

pub(super) fn pauses_new_updates(window: &Window) -> bool {
    !window.is_visible()
        && window.frame_delivery_mode() == PlatformFrameDeliveryMode::Deferred
        && window.presentation_facts().initial_presentation
            == WindowInitialPresentationStatus::Completed
}

pub(super) fn visibility_changed(
    shared: &SharedHost,
    visibility: WindowVisibility,
    window: &mut Window,
    cx: &mut App,
) {
    let mut host = shared.borrow_mut();
    let owner = window.window_handle().window_id();
    let Some(mount) = host.mounts.get(&owner).filter(|mount| mount.attached) else {
        return;
    };
    if visibility.is_visible() {
        if mount.regions.values().any(|region| region.dirty) {
            window.refresh();
        }
        return;
    }
    let Some(ticket) = mount
        .pending
        .as_ref()
        .filter(|pending| pending.update.is_some())
        .map(|pending| pending.ticket.clone())
    else {
        return;
    };
    if window.withdraw_unaccepted_frame_submission(&ticket, cx) {
        withdraw_pending(&mut host, window, cx);
        wake_waiting(&mut host, cx);
        drop(host);
        host_native::schedule(shared, cx);
    }
}

pub(super) fn observe(
    shared: &SharedHost,
    ticket: &WindowFrameSubmissionTicket,
    serial: u64,
    window: &mut Window,
) {
    let weak = Rc::downgrade(shared);
    window.on_exact_frame_submission_presented(ticket, move |snapshot, window, cx| {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let owner = window.window_handle().window_id();
        let mut host = shared.borrow_mut();
        let Some(mount) = host.mounts.get_mut(&owner) else {
            return;
        };
        if !mount
            .pending
            .as_ref()
            .is_some_and(|pending| pending.serial == serial)
        {
            return;
        }
        let pending = mount.pending.take().expect("matching pending submission");
        let exact = pending.accepted_generation.is_some()
            && snapshot.accepted_frame_generation() == pending.accepted_generation
            && snapshot.submitted_frame_generation() == pending.accepted_generation;
        let result = if snapshot.outcome() == WindowFrameSubmissionOutcome::Submitted && exact {
            SurfacePresentationResult::Presented
        } else {
            SurfacePresentationResult::Dropped
        };
        window.retire_exact_frame_submission(&pending.ticket);
        let reported_outputs = !pending.outputs.is_empty();
        retire_pending(&mut host, owner, pending, result, cx);
        if host.native.is_none() && reported_outputs {
            if let Err(error) = settle_pending(&mut host, Some(window), cx) {
                host.fail(error.as_ref());
            }
        }
        // The submitted packet was prepainted inert until this original
        // terminal observation. Replay it once to admit its content channels.
        let refresh = (reported_outputs && result == SurfacePresentationResult::Presented)
            || !host.inputs.is_empty()
            || host
                .mounts
                .get(&owner)
                .is_some_and(|mount| mount.regions.values().any(|region| region.dirty));
        drop(host);
        if refresh {
            window.refresh();
        }
        continuation::schedule(&shared, window, cx);
        host_native::schedule(&shared, cx);
    });
}

pub(super) fn accept(
    shared: &SharedHost,
    serial: u64,
    mut input_frames: Vec<InputFrame>,
    fence: AcceptedFrameFence,
    window: &mut Window,
    cx: &mut App,
) {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    if host
        .native
        .as_ref()
        .is_some_and(|native| native.is_failed())
    {
        return;
    }
    let Some(mount) = host.mounts.get_mut(&owner) else {
        return;
    };
    if serial != mount.serial {
        return;
    }
    let Some(mut pending) = mount
        .pending
        .take()
        .filter(|pending| pending.serial == serial)
    else {
        for frame in &mut input_frames {
            frame.native_binding = mount
                .input_frames
                .iter()
                .find(|previous| previous.surface == frame.surface && previous.serial == serial)
                .and_then(|previous| previous.native_binding);
        }
        mount.input_frames = input_frames;
        drop(host);
        continuation::schedule(shared, window, cx);
        return;
    };
    if pending.update.is_some()
        && (pending.input_revoked
            || pending
                .inputs
                .iter()
                .any(|input| !has_live_source(&host, input)))
    {
        // The renderer may already have finished when an input source retires.
        // The unaccepted core update still has no publication authority.
        window.retire_exact_frame_submission(&pending.ticket);
        for region in host.mounts.get_mut(&owner).unwrap().regions.values_mut() {
            region.dirty = true;
        }
        retire_pending(
            &mut host,
            owner,
            pending,
            SurfacePresentationResult::Dropped,
            cx,
        );
        if let Err(error) = settle_pending(&mut host, Some(window), cx) {
            host.fail(error.as_ref());
        }
        wake_waiting(&mut host, cx);
        window.refresh();
        return;
    }
    // A Deferred retry retains the same affine outputs and packet. Fresh GPUI
    // hitboxes must keep that packet's original native binding correlation.
    for frame in &mut input_frames {
        frame.native_binding = pending
            .input_frames
            .iter()
            .find(|previous| previous.surface == frame.surface && previous.serial == serial)
            .and_then(|previous| previous.native_binding);
    }
    pending.input_frames = input_frames;
    if let Some(update) = pending.update.take() {
        match host
            .session
            .resume_update(update)
            .and_then(|frame| frame.commit())
        {
            Ok(mut report) => {
                pending.outputs = report.take_painted_outputs();
                let released_capacity = !report.surface_presentation_commits().is_empty();
                if released_capacity {
                    wake_capacity_blocked(&mut host, cx);
                }
                if let Some(mount) = host.mounts.get_mut(&owner) {
                    for (surface, packet) in &pending.packets {
                        if packet.preserves_repaint_request()
                            && let Some(region) = mount.regions.get_mut(surface)
                        {
                            // The attempt consumed the earlier repaint. A
                            // retry only follows invalidation from this report.
                            region.dirty = false;
                        }
                    }
                }
                host.apply_report(report);
                host_native::provisional::accept_preview(&mut host, owner, &pending.packets);
                pending.inputs.clear();
                if let Some(mount) = host.mounts.get_mut(&owner) {
                    for (surface, packet) in &pending.packets {
                        if let Some(region) = mount.regions.get_mut(surface) {
                            region.packet = Some(packet.clone());
                            if !packet.preserves_repaint_request() {
                                region.dirty = false;
                            }
                        }
                    }
                }
            }
            Err(error) => {
                pending.inputs.append(&mut host.inputs);
                host.inputs = pending.inputs;
                host.fail(&error);
                window.retire_exact_frame_submission(&pending.ticket);
                window.refresh();
                return;
            }
        }
    }
    for frame in &mut pending.input_frames {
        if frame.native_binding.is_none() {
            frame.native_binding = pending
                .outputs
                .iter()
                .find(|output| output.surface() == frame.surface)
                .and_then(|output| host_native::output_binding(&host, owner, output));
        }
    }
    pending.accepted_generation = Some(fence.generation());
    host.mounts.get_mut(&owner).expect("live mount").pending = Some(pending);
    drop(host);
    continuation::schedule(shared, window, cx);
}

pub(super) fn accept_focus_stable(
    shared: &SharedHost,
    serial: u64,
    window: &mut Window,
    cx: &mut App,
) {
    let owner = window.window_handle().window_id();
    let host = shared.borrow();
    let Some(pending) = host
        .mounts
        .get(&owner)
        .and_then(|mount| mount.pending.as_ref())
        .filter(|pending| pending.serial == serial)
    else {
        return;
    };
    if let Some(generation) = pending.accepted_generation {
        if let Err(error) = host_native::provisional::accept_submission(
            &host,
            &pending.ticket,
            generation,
            window,
            cx,
        ) {
            log::error!("GPUI docking exact submission: {error:#}");
            window.refresh();
        }
    }
    drop(host);
    {
        let mut host = shared.borrow_mut();
        if let Err(error) = settle_pending(&mut host, Some(window), cx) {
            host.fail(error.as_ref());
        }
        wake_waiting(&mut host, cx);
    }
    host_native::schedule(shared, cx);
}

pub(super) fn discard(shared: &SharedHost, window: &mut Window, cx: &mut App) {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    let Some(mount) = host.mounts.get_mut(&owner) else {
        return;
    };
    let capture = mount.capture;
    mount.attached = false;
    mount.wake = None;
    mount.input_frames.clear();
    for region in mount.regions.values_mut() {
        region.dirty = true;
        region.presented = None;
        region.presented_panes.clear();
    }
    withdraw_pending(&mut host, window, cx);
    retire_input_source(&mut host, owner, cx);
    if let Err(error) = settle_pending(&mut host, Some(window), cx) {
        host.fail(error.as_ref());
    }
    wake_waiting(&mut host, cx);
    drop(host);
    let _ = window.release_pointer(&capture);
    host_native::schedule(shared, cx);
}

fn has_live_source(host: &HostState, input: &QueuedInput) -> bool {
    host.mounts
        .get(&input.owner())
        .is_some_and(|mount| mount.attached)
}

fn retire_input_source(host: &mut HostState, owner: WindowId, cx: &mut App) {
    host.inputs.retain(|input| input.owner() != owner);
    // A surviving window can hold the unaccepted update that borrowed this
    // mount's input. Withdraw it if possible and retain the other inputs in order.
    let borrowed = host.mounts.values_mut().find_map(|mount| {
        let pending = mount.pending.as_mut()?;
        pending
            .revoke_inputs_from(owner)
            .then(|| (mount.window, pending.ticket.clone()))
    });
    if let Some((borrower, ticket)) = borrowed {
        let _ = borrower.update(cx, |_, window, cx| {
            if window.withdraw_unaccepted_frame_submission(&ticket, cx) {
                withdraw_pending(host, window, cx);
            }
        });
    }
}

fn withdraw_pending(host: &mut HostState, window: &mut Window, cx: &mut App) {
    let owner = window.window_handle().window_id();
    let Some(mount) = host.mounts.get_mut(&owner) else {
        return;
    };
    mount.wake = None;
    for region in mount.regions.values_mut() {
        region.dirty = true;
    }
    let Some(pending) = mount.pending.take() else {
        return;
    };
    window.retire_exact_frame_submission(&pending.ticket);
    retire_pending(host, owner, pending, SurfacePresentationResult::Dropped, cx);
}

fn retire_pending(
    host: &mut HostState,
    owner: WindowId,
    mut pending: PendingSubmission,
    result: SurfacePresentationResult,
    cx: &mut App,
) {
    if pending.update.take().is_some() {
        pending.inputs.retain(|input| has_live_source(host, input));
        pending.inputs.append(&mut host.inputs);
        host.inputs = pending.inputs;
        // An unaccepted update has never published any affine outputs.
        debug_assert!(pending.outputs.is_empty());
        return;
    }
    let terminal = PendingPresentation::Submission {
        owner,
        serial: pending.serial,
        outputs: pending.outputs,
        input_frames: pending.input_frames,
        result,
    };
    if let Some(native) = &host.native {
        // Preserve the original affine outputs at their arrival boundary. Native
        // ingress before this terminal commits first; its suffix stays queued.
        native.transport.presentation(terminal, cx);
    } else {
        apply_presentation(host, terminal);
    }
}

fn apply_presentation(host: &mut HostState, terminal: PendingPresentation) {
    let (owner, serial, outputs, input_frames, result) = match terminal {
        PendingPresentation::MountRemoved {
            owner,
            mut surfaces,
        } => {
            surfaces.retain(|surface| {
                host.owners
                    .get(surface)
                    .is_none_or(|current| *current == owner)
            });
            host.unavailable_surfaces.extend(surfaces);
            return;
        }
        PendingPresentation::Submission {
            owner,
            serial,
            outputs,
            input_frames,
            result,
        } => (owner, serial, outputs, input_frames, result),
    };
    let mut panes: BTreeMap<_, _> = input_frames
        .iter()
        .map(|frame| (frame.surface, frame.panes.clone()))
        .collect();
    if result == SurfacePresentationResult::Presented
        && let Some(mount) = host
            .mounts
            .get_mut(&owner)
            .filter(|mount| mount.serial == serial)
    {
        for frame in &input_frames {
            if frame.panes.is_empty()
                && let Some(region) = mount.regions.get_mut(&frame.surface)
            {
                region.presented_panes.clear();
            }
        }
        mount.input_frames = input_frames;
    }
    host.presentation_pending |= !outputs.is_empty();
    for output in outputs {
        let surface = output.surface();
        let artifact = output.presentation_artifact();
        match host.session.report_surface_presentation(output, result) {
            Ok(()) => {
                if let Some(region) = host
                    .mounts
                    .get_mut(&owner)
                    .filter(|mount| mount.serial == serial)
                    .and_then(|mount| mount.regions.get_mut(&surface))
                {
                    if result == SurfacePresentationResult::Presented {
                        region.presented = Some((serial, artifact));
                        region.presented_panes = panes
                            .remove(&surface)
                            .unwrap_or_default()
                            .into_iter()
                            .collect();
                    } else {
                        region.dirty = true;
                    }
                }
            }
            Err(error) => host.fail(&error),
        }
    }
}

pub(super) fn settle_native(
    host: &mut HostState,
    terminal: PendingPresentation,
    window: Option<&Window>,
    cx: &mut App,
) -> anyhow::Result<()> {
    apply_presentation(host, terminal);
    settle_pending(host, window, cx)
}

pub(super) fn settle_pending(
    host: &mut HostState,
    window: Option<&Window>,
    cx: &mut App,
) -> anyhow::Result<()> {
    if (!host.presentation_pending && host.unavailable_surfaces.is_empty())
        || host.has_unaccepted_update()
    {
        return Ok(());
    }
    let unavailable: Vec<_> = host
        .unavailable_surfaces
        .iter()
        .copied()
        .filter(|surface| {
            !host
                .owners
                .get(surface)
                .and_then(|owner| host.mounts.get(owner))
                .is_some_and(|mount| mount.attached)
        })
        .collect();
    let mut frame = host_native::begin_frame(
        &mut host.session,
        &host.mounts,
        host.native.as_ref(),
        window,
        cx,
    )?;
    let roster = frame.surfaces();
    for surface in unavailable {
        if roster.contains(&surface) {
            // A preserved logical surface has no measuring/rendering owner now.
            // Explicit unavailability revokes the old scene's interaction authority;
            // dropping outputs alone does not withdraw a previously displayed scene.
            frame.defer_surface(surface, SurfaceUnavailableReason::SurfaceUnavailable)?;
        }
    }
    let report = frame.commit()?;
    let released_capacity = !report.surface_presentation_commits().is_empty();
    if released_capacity {
        wake_capacity_blocked(host, cx);
    }
    host.apply_report(report);
    host.presentation_pending = false;
    host.unavailable_surfaces.clear();
    Ok(())
}

pub(super) fn wake_capacity_blocked(host: &mut HostState, cx: &mut App) {
    // Only a committed presentation prefix releases output capacity. Waking on
    // renderer acceptance or arbitrary native work would turn dirt into polling.
    for mount in host.mounts.values_mut() {
        if std::mem::take(&mut mount.capacity_blocked)
            && mount.attached
            && mount.pending.is_none()
            && mount.regions.values().any(|region| region.dirty)
        {
            let window = mount.window;
            cx.defer(move |cx| {
                let _ = window.update(cx, |_, window, _| window.refresh());
            });
        }
    }
}

fn wake_waiting(host: &mut HostState, cx: &mut App) {
    if host.has_unaccepted_update() {
        return;
    }
    let has_local_input = host.native.is_none() && !host.inputs.is_empty();
    for mount in host.mounts.values_mut() {
        let waiting = std::mem::take(&mut mount.waiting_for_update);
        if waiting || (has_local_input && mount.attached && mount.pending.is_none()) {
            let window = mount.window;
            cx.defer(move |cx| {
                let _ = window.update(cx, |_, window, _| {
                    if waiting || window.is_visible() {
                        window.refresh();
                    }
                });
            });
        }
    }
}

pub(super) fn close(shared: &SharedHost, owner: WindowId, cx: &mut App) {
    retire_mount(&mut shared.borrow_mut(), owner, cx);
    host_native::schedule(shared, cx);
}

pub(super) fn retire_mount(host: &mut HostState, owner: WindowId, cx: &mut App) {
    if let Some(mut mount) = host.mounts.remove(&owner) {
        if let Some(pending) = mount.pending.take() {
            retire_pending(host, owner, pending, SurfacePresentationResult::Dropped, cx);
        }
        retire_input_source(host, owner, cx);
        let mut surfaces = Vec::new();
        for surface in mount.regions.keys() {
            if host
                .owners
                .get(surface)
                .is_none_or(|current| *current == owner)
            {
                host.owners.remove(surface);
                surfaces.push(*surface);
            }
        }
        let removed = PendingPresentation::MountRemoved { owner, surfaces };
        if let Some(native) = &host.native {
            // Keep actual mount loss behind the same original native prefix as
            // renderer terminals. A rejected head cannot be bypassed by cleanup.
            native.transport.presentation(removed, cx);
        } else {
            apply_presentation(host, removed);
        }
    }
    if let Err(error) = settle_pending(host, None, cx) {
        host.fail(error.as_ref());
    }
    wake_waiting(host, cx);
}
