//! One exact GPUI ticket covers every logical region in the physical frame.

use std::{collections::BTreeMap, rc::Rc};

use dockspace::{
    model::SurfaceId,
    runtime::{DockspaceUpdate, PaintedSurfaceOutput, SurfacePresentationResult},
};
use open_gpui::{
    AcceptedFrameFence, App, Window, WindowFrameSubmissionOutcome, WindowFrameSubmissionTicket,
    WindowId,
};

use super::{
    HostState, SharedHost, continuation, element::Packet, host_native, input::QueuedPointer,
};

pub(super) struct PendingSubmission {
    pub(super) serial: u64,
    pub(super) ticket: WindowFrameSubmissionTicket,
    pub(super) packets: BTreeMap<SurfaceId, Rc<Packet>>,
    pub(super) update: Option<DockspaceUpdate>,
    pub(super) inputs: Vec<QueuedPointer>,
    outputs: Vec<PaintedSurfaceOutput>,
    accepted_generation: Option<u64>,
    input_frames: Vec<continuation::InputFrame>,
}

impl PendingSubmission {
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
        inputs: Vec<QueuedPointer>,
    ) -> Self {
        Self {
            serial,
            ticket,
            packets,
            update: Some(update),
            inputs,
            outputs: Vec::new(),
            accepted_generation: None,
            input_frames: Vec::new(),
        }
    }
}

pub(super) fn abandon_unaccepted(host: &mut HostState, window: &mut Window) {
    let owner = window.window_handle().window_id();
    if host
        .mounts
        .get(&owner)
        .and_then(|mount| mount.pending.as_ref())
        .is_some_and(|pending| pending.update.is_some())
    {
        withdraw_pending(host, window);
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
        let mut pending = mount.pending.take().expect("matching pending submission");
        let exact = pending.accepted_generation.is_some()
            && snapshot.accepted_frame_generation() == pending.accepted_generation
            && snapshot.submitted_frame_generation() == pending.accepted_generation;
        let result = if snapshot.outcome() == WindowFrameSubmissionOutcome::Submitted && exact {
            SurfacePresentationResult::Presented
        } else {
            SurfacePresentationResult::Dropped
        };
        window.retire_exact_frame_submission(&pending.ticket);
        if result == SurfacePresentationResult::Presented {
            if let Some(mount) = host.mounts.get_mut(&owner) {
                mount.input_frames = pending.input_frames;
            }
        }
        let reported_outputs = !pending.outputs.is_empty();
        for output in pending.outputs.drain(..) {
            let surface = output.surface();
            let artifact = output.presentation_artifact();
            match host.session.report_surface_presentation(output, result) {
                Ok(()) => {
                    if let Some(region) = host
                        .mounts
                        .get_mut(&owner)
                        .and_then(|mount| mount.regions.get_mut(&surface))
                    {
                        if result == SurfacePresentationResult::Presented {
                            region.presented = Some((serial, artifact));
                        } else {
                            region.dirty = true;
                        }
                    }
                }
                Err(error) => host.fail(error),
            }
        }
        if reported_outputs {
            host.presentation_pending = true;
            if let Err(error) = settle_pending(&mut host, Some(window), cx) {
                host.fail(error);
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
    mut input_frames: Vec<continuation::InputFrame>,
    fence: AcceptedFrameFence,
    window: &mut Window,
    cx: &App,
) {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
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
                host.apply_report(report);
                pending.inputs.clear();
                if let Some(mount) = host.mounts.get_mut(&owner) {
                    for (surface, packet) in &pending.packets {
                        if let Some(region) = mount.regions.get_mut(surface) {
                            region.packet = Some(packet.clone());
                            // This exact candidate already encoded the report's
                            // current scene. Other physical mounts stay dirty.
                            // Submission still owns presentation admission.
                            region.dirty = false;
                        }
                    }
                }
            }
            Err(error) => {
                pending.inputs.append(&mut host.inputs);
                host.inputs = pending.inputs;
                host.fail(error);
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
        if let Err(error) =
            window.accept_exact_frame_submission_frame(&pending.ticket, generation, cx)
        {
            log::error!("GPUI docking exact submission: {error:#}");
            window.refresh();
        }
    }
    drop(host);
    {
        let mut host = shared.borrow_mut();
        if let Err(error) = settle_pending(&mut host, Some(window), cx) {
            host.fail(error);
        }
        wake_waiting(&mut host, cx);
    }
    host_native::schedule(shared, cx);
}

pub(super) fn discard(shared: &SharedHost, window: &mut Window, cx: &mut App) {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    withdraw_pending(&mut host, window);
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
    }
    if let Err(error) = settle_pending(&mut host, Some(window), cx) {
        host.fail(error);
    }
    wake_waiting(&mut host, cx);
    drop(host);
    let _ = window.release_pointer(&capture);
    host_native::schedule(shared, cx);
}

fn withdraw_pending(host: &mut HostState, window: &mut Window) {
    let owner = window.window_handle().window_id();
    let Some(mount) = host.mounts.get_mut(&owner) else {
        return;
    };
    mount.wake = None;
    for region in mount.regions.values_mut() {
        region.dirty = true;
    }
    let Some(mut pending) = mount.pending.take() else {
        return;
    };
    window.retire_exact_frame_submission(&pending.ticket);
    let unaccepted = pending.update.take().is_some();
    host.presentation_pending |= !pending.outputs.is_empty();
    for output in pending.outputs.drain(..) {
        if let Err(error) = host
            .session
            .report_surface_presentation(output, SurfacePresentationResult::Dropped)
        {
            host.fail(error);
        }
    }
    if unaccepted {
        pending.inputs.append(&mut host.inputs);
        host.inputs = pending.inputs;
    }
}

fn settle_pending(
    host: &mut HostState,
    window: Option<&Window>,
    cx: &mut App,
) -> anyhow::Result<()> {
    if !host.presentation_pending || host.has_unaccepted_update() {
        return Ok(());
    }
    let frame = host_native::begin_frame(
        &mut host.session,
        &host.mounts,
        host.native.as_ref(),
        window,
        cx,
    )?;
    let report = frame.commit()?;
    host.apply_report(report);
    host.presentation_pending = false;
    Ok(())
}

fn wake_waiting(host: &mut HostState, cx: &mut App) {
    if host.has_unaccepted_update() {
        return;
    }
    for mount in host.mounts.values_mut() {
        if std::mem::take(&mut mount.waiting_for_update) {
            let window = mount.window;
            cx.defer(move |cx| {
                let _ = window.update(cx, |_, window, _| window.refresh());
            });
        }
    }
}

pub(super) fn close(host: &mut HostState, owner: WindowId, cx: &mut App) {
    if let Some(mut mount) = host.mounts.remove(&owner) {
        if let Some(mut pending) = mount.pending.take() {
            drop(pending.update.take());
            host.presentation_pending |= !pending.outputs.is_empty();
            for output in pending.outputs.drain(..) {
                if let Err(error) = host
                    .session
                    .report_surface_presentation(output, SurfacePresentationResult::Dropped)
                {
                    host.fail(error);
                }
            }
        }
        for surface in mount.regions.keys() {
            host.owners.remove(surface);
        }
    }
    if let Err(error) = settle_pending(host, None, cx) {
        host.fail(error);
    }
    wake_waiting(host, cx);
}
