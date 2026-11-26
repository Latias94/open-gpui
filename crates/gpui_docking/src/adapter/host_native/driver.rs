//! Two-phase native pump, collection, report dispatch, and continuation scheduling.
//!
//! Every `AnyWindowHandle::update` side effect discovered while mutably borrowing
//! `SharedHost` is collected into [`DeferredHostWork`] and flushed strictly after
//! releasing `RefMut<HostState>`, preventing reentrant `RefCell` borrow panics.

use std::rc::Rc;

use anyhow::Result;
use dockspace::runtime::{
    HostWindowToken, NativeHostErrorKind, NativeSurfaceCloseAction, NativeWindowBinding,
};
use open_gpui::{AnyWindowHandle, App, Window, WindowId};

use super::super::{
    DockSurface, HostState, SharedHost,
    native::{NativeBridge, NativeBridgeError, NativeCallbacks, NativeCloseDecision, NativeReport},
    presentation,
};
use super::{
    ManagedHost, NativeWake, QuiescenceCallback, begin_frame, default_factory, is_quiescent,
    provisional, termination,
};

/// Side effects that must invoke GPUI `AnyWindowHandle::update` or user callbacks
/// strictly outside any `SharedHost` borrow.
#[derive(Default)]
#[must_use = "flush deferred host work after releasing the SharedHost borrow"]
pub(super) struct DeferredHostWork {
    pub(super) root_binds: Vec<(WindowId, NativeWindowBinding, AnyWindowHandle, bool)>,
    pub(super) borrowed_submissions: Vec<(AnyWindowHandle, open_gpui::WindowFrameSubmissionTicket)>,
    refresh_windows: Vec<AnyWindowHandle>,
    quiescence_callbacks: Vec<QuiescenceCallback>,
    termination_callbacks: Vec<termination::TerminationCallback>,
}

impl DeferredHostWork {
    /// Flushes deferred root window binds and borrowed submission withdrawals
    /// before dispatching native effects or returning the bridge.
    ///
    /// Callers must have dropped every `SharedHost` borrow before calling this.
    pub(super) fn flush_pre_dispatch(
        &mut self,
        shared: &SharedHost,
        bridge: &mut NativeBridge,
        cx: &mut App,
    ) -> bool {
        debug_assert!(
            shared.try_borrow_mut().is_ok(),
            "DeferredHostWork::flush_pre_dispatch called while SharedHost is borrowed"
        );
        let bound = if self.root_binds.is_empty() {
            false
        } else {
            let pending = std::mem::take(&mut self.root_binds);
            complete_deferred_root_binds(shared, bridge, pending, cx)
        };
        if !self.borrowed_submissions.is_empty() {
            let submissions = std::mem::take(&mut self.borrowed_submissions);
            presentation::withdraw_borrowed_submissions(shared, submissions, cx);
        }
        bound
    }

    /// Flushes remaining borrowed submissions, window refreshes, notifications,
    /// pump rescheduling, and quiescence/termination callbacks strictly outside
    /// any `SharedHost` borrow.
    pub(super) fn flush_post_pump(self, shared: &SharedHost, schedule_more: bool, cx: &mut App) {
        debug_assert!(
            shared.try_borrow_mut().is_ok(),
            "DeferredHostWork::flush_post_pump called while SharedHost is borrowed"
        );
        debug_assert!(
            self.root_binds.is_empty(),
            "deferred root binds must flush while the bridge is checked out"
        );
        presentation::withdraw_borrowed_submissions(shared, self.borrowed_submissions, cx);
        for window in self.refresh_windows {
            let _ = window.update(cx, |_, window, _| window.refresh());
        }
        super::super::notifications::schedule(shared, cx);
        if schedule_more || super::super::close::ready(&shared.borrow()) {
            schedule(shared, cx);
        }
        for callback in self.quiescence_callbacks {
            callback(cx);
        }
        for callback in self.termination_callbacks {
            callback(super::super::DockNativeTermination::Drained, cx);
        }
    }
}

pub(super) fn complete_deferred_root_binds(
    shared: &SharedHost,
    bridge: &mut NativeBridge,
    pending: Vec<(WindowId, NativeWindowBinding, AnyWindowHandle, bool)>,
    cx: &mut App,
) -> bool {
    debug_assert!(
        shared.try_borrow_mut().is_ok(),
        "complete_deferred_root_binds called while SharedHost is borrowed"
    );
    let mut bound = false;
    for (owner, binding, window_handle, anchor) in pending {
        let result = window_handle.update(cx, |_, window, cx| {
            bridge.bind_window_current(binding, window, anchor, cx)
        });
        let mut host = shared.borrow_mut();
        match result {
            Ok(Ok(callbacks)) => {
                if let Some(native) = host.native.as_mut() {
                    native.callbacks.insert(owner, callbacks);
                    native.roots.insert(owner);
                    native.registrations.remove(&owner);
                    bound = true;
                }
            }
            Ok(Err(error)) => host.fail(&error),
            Err(error) => host.fail(error.as_ref()),
        }
    }
    bound
}

pub(in crate::adapter) fn prepare_window(shared: &SharedHost, window: &mut Window, cx: &mut App) {
    let mut deferred = DeferredHostWork::default();
    let (mut bridge, mut more) = {
        let mut host = shared.borrow_mut();
        let Some(native) = host.native.as_mut() else {
            return;
        };
        let owner = window.window_handle().window_id();
        // A native factory may draw its root before returning its handle. The bridge
        // is checked out for dispatch, so retain that original callback route here
        // before the element validates the not-yet-committed staging surface.
        if let Some(callbacks) = native
            .factory_opening
            .as_ref()
            .filter(|callbacks| callbacks.window_id() == Some(owner))
        {
            native
                .callbacks
                .entry(owner)
                .or_insert_with(|| callbacks.clone());
        }
        if let Some(callbacks) = native.callbacks.get(&owner) {
            callbacks.observe(window, cx);
        }
        let Some(mut bridge) = native.bridge.take() else {
            return;
        };
        let more = match collect_host(&mut host, &mut bridge, Some(window), &mut deferred, cx) {
            Ok(more) => more,
            Err(error) => {
                host.fail(error.as_ref());
                false
            }
        };
        (bridge, more)
    };
    more |= deferred.flush_pre_dispatch(shared, &mut bridge, cx);
    {
        let mut host = shared.borrow_mut();
        host.native.as_mut().unwrap().bridge = Some(bridge);
        schedule_continuation(shared, &mut host, cx);
    }
    if more {
        schedule(shared, cx);
    }
}

pub(in crate::adapter) fn schedule(shared: &SharedHost, cx: &mut App) {
    super::super::notifications::schedule(shared, cx);
    {
        let mut host = shared.borrow_mut();
        let scheduled = if let Some(native) = &mut host.native {
            &mut native.pump_scheduled
        } else {
            if !super::super::close::ready(&host) {
                return;
            }
            &mut host.close_pump_scheduled
        };
        if std::mem::replace(scheduled, true) {
            return;
        }
    }
    let weak = Rc::downgrade(shared);
    cx.defer_shutdown_critical_before_window_registry_clear_or_run_now(move |cx| {
        if let Some(shared) = weak.upgrade() {
            {
                let mut host = shared.borrow_mut();
                if let Some(native) = &mut host.native {
                    native.pump_scheduled = false;
                } else {
                    host.close_pump_scheduled = false;
                }
            }
            pump(&shared, cx);
        }
    });
}

pub(super) fn collect_host(
    host: &mut HostState,
    bridge: &mut NativeBridge,
    current: Option<&Window>,
    deferred: &mut DeferredHostWork,
    cx: &mut App,
) -> Result<bool> {
    if host
        .native
        .as_ref()
        .is_some_and(|native| native.failed_shutdown.is_some())
    {
        return Ok(false);
    }
    // A prepared GPUI candidate still owns its rollbackable semantic update.
    // Preserve ingress ordering until that exact candidate is accepted or
    // withdrawn; collecting a native prefix here would require another update.
    if host.has_unaccepted_update() {
        return Ok(false);
    }
    presentation::settle_pending(host, current, cx)?;
    while let Some(report) = host.native.as_mut().unwrap().pending_reports.pop_front() {
        bridge.accept_report(report);
    }
    recover_root_registrations(host, bridge, current, &mut deferred.root_binds, cx)?;
    for _ in 0..4 {
        if let Err(error) = bridge.collect(&mut host.session) {
            if !host.session.has_pending_native_input() {
                return Err(error.into());
            }
        }
        process_reports(host, bridge, &mut deferred.borrowed_submissions, cx)?;
        let pending_input = host.session.has_pending_native_input();
        if !pending_input && let Some(action) = bridge.transport().pending_surface_action() {
            // This semantic consequence follows its original release in the
            // same FIFO. Retain the head until its own frame commits; neither
            // native suffixes nor renderer retries may replay or overtake it.
            let mut frame = begin_frame(
                &mut host.session,
                &host.mounts,
                host.native.as_ref(),
                current,
                cx,
            )?;
            frame.submit_deferred_surface_action(action)?;
            let report = frame.commit()?;
            bridge.transport().commit_surface_action();
            host.invalidate_report(&report);
            bridge.accept_report(report);
            process_reports(host, bridge, &mut deferred.borrowed_submissions, cx)?;
            continue;
        }
        if !pending_input && let Some(terminal) = bridge.transport().take_presentation() {
            presentation::settle_native(host, terminal, current, cx)?;
            while let Some(report) = host.native.as_mut().unwrap().pending_reports.pop_front() {
                bridge.accept_report(report);
            }
            process_reports(host, bridge, &mut deferred.borrowed_submissions, cx)?;
            continue;
        }
        let may_create = !host.native.as_ref().unwrap().transport.is_frozen()
            && host.session.has_pending_native_create();
        if !pending_input && !may_create {
            return Ok(false);
        }
        let HostState {
            session,
            mounts,
            native,
            ..
        } = host;
        let mut frame = begin_frame(session, mounts, native.as_ref(), current, cx)?;
        let advanced = may_create && frame.advance_next_native_surface_create()?;
        if !pending_input && !advanced {
            return Ok(false);
        }
        let report = frame.commit()?;
        let progressed = advanced || !report.inputs().is_empty();
        host.invalidate_report(&report);
        bridge.accept_report(report);
        recover_root_registrations(host, bridge, current, &mut deferred.root_binds, cx)?;
        process_reports(host, bridge, &mut deferred.borrowed_submissions, cx)?;
        if !progressed {
            // Effect results can change the roster without a public input outcome.
            return Ok(bridge.republish_inventory_if_complete(&mut host.session)?);
        }
    }
    Ok(host.session.has_pending_native_input()
        || !bridge.transport().is_empty()
        || bridge.republish_inventory_if_complete(&mut host.session)?)
}

fn recover_root_registrations(
    host: &mut HostState,
    bridge: &mut NativeBridge,
    current: Option<&Window>,
    pending_root_binds: &mut Vec<(WindowId, NativeWindowBinding, AnyWindowHandle, bool)>,
    cx: &mut App,
) -> Result<()> {
    let owners: Vec<_> = host
        .native
        .as_ref()
        .unwrap()
        .registrations
        .keys()
        .copied()
        .collect();
    for owner in owners {
        let surface = host.native.as_ref().unwrap().registrations[&owner].surface;
        let Some(binding) = host
            .session
            .native_window_bindings()
            .find(|binding| binding.window_token() == HostWindowToken::new(owner.as_u64()))
        else {
            // A committed semantic rejection leaves no binding to retire.
            if !host.session.has_pending_native_input() {
                host.native.as_mut().unwrap().registrations.remove(&owner);
            }
            continue;
        };
        if !host
            .session
            .native_surface_bindings_for_window(binding)
            .any(|binding| binding.surface() == surface)
        {
            continue;
        }
        let native = host.native.as_mut().unwrap();
        let registration = &native.registrations[&owner];
        let ticket = native
            .retirements
            .get(&owner)
            .cloned()
            .or_else(|| registration.retirement.ticket());
        let callbacks = if let Some(ticket) = ticket {
            bridge.retain_root_rollback(binding, ticket, registration.anchor, cx)?
        } else if let Some(callbacks) = bridge.callbacks_for_window(owner) {
            callbacks
        } else {
            // The original construction Window cannot be checked out again, and
            // updating another window handle must run outside the SharedHost borrow.
            if current.is_some_and(|window| window.window_handle().window_id() == owner) {
                continue;
            }
            if !pending_root_binds
                .iter()
                .any(|(queued, _, _, _)| *queued == owner)
            {
                pending_root_binds.push((owner, binding, registration.window, registration.anchor));
            }
            continue;
        };
        native.callbacks.insert(owner, callbacks);
        native.roots.insert(owner);
        native.registrations.remove(&owner);
    }
    Ok(())
}

pub(super) fn process_reports(
    host: &mut HostState,
    bridge: &mut NativeBridge,
    borrowed_submissions: &mut Vec<(AnyWindowHandle, open_gpui::WindowFrameSubmissionTicket)>,
    cx: &mut App,
) -> Result<()> {
    while let Some(mut report) = bridge.pop_report() {
        let mut deferred_close = false;
        match &report {
            NativeReport::CapturedRelease { capture, frame } => {
                provisional::observe_release(host.native.as_mut().unwrap(), *capture, *frame);
            }
            NativeReport::Admitted(binding) => {
                provisional::observe_admitted(host.native.as_mut().unwrap(), *binding, cx);
            }
            NativeReport::CloseRequested(close) => {
                let native = host.native.as_ref().unwrap();
                let root = native.callbacks.iter().any(|(owner, callbacks)| {
                    native.roots.contains(owner) && callbacks.binding() == close.binding().window()
                });
                let policy = native
                    .close_policy
                    .unwrap_or(NativeCloseDecision::Allow(if root {
                        NativeSurfaceCloseAction::RetainLayout
                    } else {
                        NativeSurfaceCloseAction::RecoverPresentation
                    }));
                deferred_close =
                    matches!(policy, NativeCloseDecision::Defer) && !native.transport.is_frozen();
                if !host.native.as_ref().unwrap().transport.is_frozen()
                    && let Err(error) = bridge.decide_close(&mut host.session, *close, policy)
                {
                    if matches!(&error, NativeBridgeError::Core(error)
                        if error.native_kind() == Some(NativeHostErrorKind::StaleBinding))
                        && host.session.recognizes_native_binding(close.binding())
                        && !host.session.is_current_native_binding(close.binding())
                    {
                        report = NativeReport::StaleCallback(close.binding().window());
                    } else {
                        bridge.retry_report(report);
                        return Err(error.into());
                    }
                }
            }
            NativeReport::LogicalClosed(binding) => {
                if let Some(owner) =
                    host.native
                        .as_ref()
                        .unwrap()
                        .callbacks
                        .iter()
                        .find_map(|(owner, callbacks)| {
                            (callbacks.binding() == *binding).then_some(*owner)
                        })
                {
                    if let Some(borrowed) = presentation::retire_mount(host, owner, cx) {
                        borrowed_submissions.push(borrowed);
                    }
                }
            }
            NativeReport::NativeTerminal(binding) => {
                let retired: Vec<_> = host
                    .session
                    .retained_native_surface_bindings_for_window(*binding)
                    .collect();
                host.native.as_mut().unwrap().retired.extend(retired);
            }
            NativeReport::RetirePlacement(binding) => {
                host.native.as_mut().unwrap().retired.insert(*binding);
            }
            NativeReport::OpenFailed { message, .. } => {
                log::warn!("native docking create failed: {message}")
            }
            _ => {}
        }
        let reports = &mut host.native.as_mut().unwrap().reports;
        match report {
            NativeReport::CloseRequested(close) if deferred_close => {
                reports.push_deferred_close(close);
            }
            report => reports.push_back(report),
        }
    }
    Ok(())
}

pub(in crate::adapter) fn pump(shared: &SharedHost, cx: &mut App) {
    super::super::notifications::schedule(shared, cx);
    if shared.try_borrow().is_ok_and(|host| {
        host.native
            .as_ref()
            .is_some_and(|native| native.failed_shutdown.is_some())
    }) {
        termination::pump_failed(shared, cx);
        return;
    }
    super::super::close::dispatch(shared, cx);

    // Phase 1: borrow SharedHost, check out bridge, collect native ingress/reports.
    let mut deferred = DeferredHostWork::default();
    let (mut bridge, factory, mut more) = {
        let Ok(mut host) = shared.try_borrow_mut() else {
            return;
        };
        let Some(native) = host.native.as_mut() else {
            return;
        };
        let Some(mut bridge) = native.bridge.take() else {
            return;
        };
        native.retirements.retain(|owner, ticket| {
            let snapshot = ticket.snapshot();
            native.registrations.contains_key(owner)
                || !(snapshot.quiesced() && snapshot.native_terminal())
        });
        let factory = native.factory.clone();
        let more = match collect_host(&mut host, &mut bridge, None, &mut deferred, cx) {
            Ok(more) => more,
            Err(error) => {
                host.fail(error.as_ref());
                false
            }
        };
        while let Some(report) = host.native.as_mut().unwrap().pending_reports.pop_front() {
            bridge.accept_report(report);
        }
        (bridge, factory, more)
    };

    // Phase 2 (no SharedHost borrow): flush deferred binds/withdrawals and dispatch OS effects.
    more |= deferred.flush_pre_dispatch(shared, &mut bridge, cx);
    let dock = DockSurface {
        host: shared.clone(),
    };
    if provisional::drive(shared, &bridge, cx) {
        if let Err(error) = bridge.dispatch_effects(
            &mut |request, callbacks, cx| {
                shared.borrow_mut().native.as_mut().unwrap().factory_opening =
                    Some(callbacks.clone());
                let opened = if let Some(factory) = &factory {
                    factory(&dock, request, callbacks, cx)
                } else {
                    default_factory(&dock, request, callbacks, cx)
                };
                let mut host = shared.borrow_mut();
                let native = host.native.as_mut().unwrap();
                native.factory_opening = None;
                if let Some(owner) = callbacks.window_id() {
                    native
                        .callbacks
                        .entry(owner)
                        .or_insert_with(|| callbacks.clone());
                }
                opened
            },
            cx,
        ) {
            shared.borrow_mut().fail(&error);
        }
    }
    if let Err(error) = bridge.drive_shutdown(cx) {
        shared.borrow_mut().fail(&error);
    }

    // Phase 3: return bridge, settle post-dispatch reports and quiescence, collect post-pump work.
    {
        let mut host = shared.borrow_mut();
        while let Some(report) = host.native.as_mut().unwrap().pending_reports.pop_front() {
            bridge.accept_report(report);
            more = true;
        }
        if let Err(error) = process_reports(
            &mut host,
            &mut bridge,
            &mut deferred.borrowed_submissions,
            cx,
        ) {
            host.fail(error.as_ref());
        }
        if !host.session.has_pending_native_input() && host.unavailable_surfaces.is_empty() {
            let retired: Vec<_> = host
                .native
                .as_ref()
                .unwrap()
                .retired
                .iter()
                .copied()
                .collect();
            for binding in retired {
                let retained = host.mounts.values().any(|mount| {
                    mount
                        .input_frames
                        .iter()
                        .any(|frame| frame.native_binding == Some(binding))
                        || mount
                            .pending
                            .as_ref()
                            .is_some_and(|pending| pending.outputs_for_binding(binding))
                });
                if retained {
                    continue;
                }
                if let Some(callbacks) = bridge.callbacks(binding.window()) {
                    // A placement may retire while the physical root continues.
                    // Its live successor keeps the callback installation unsealed.
                    let _ = callbacks.seal();
                }
                match bridge.quiesce_binding(&mut host.session, binding) {
                    Ok(()) => {
                        host.native.as_mut().unwrap().retired.remove(&binding);
                        if bridge.callbacks(binding.window()).is_none() {
                            let ManagedHost {
                                callbacks, roots, ..
                            } = host.native.as_mut().unwrap();
                            callbacks.retain(|owner, callbacks| {
                                let retained = callbacks.binding() != binding.window();
                                if !retained {
                                    roots.remove(owner);
                                }
                                retained
                            });
                        }
                        more = true;
                    }
                    Err(NativeBridgeError::NotQuiescent) => {}
                    Err(error) => host.fail(&error),
                }
            }
        }
        host.native.as_mut().unwrap().bridge = Some(bridge);
        schedule_continuation(shared, &mut host, cx);
        deferred.refresh_windows = host
            .mounts
            .values()
            // An emitted output waiting on its renderer is a continuation, not a
            // new paint request. Its actual terminal callback wakes retained dirt.
            .filter(|mount| {
                !mount.capacity_blocked
                    && mount.pending.is_none()
                    && mount.regions.values().any(|region| region.dirty)
            })
            .map(|mount| mount.window)
            .collect();
        if is_quiescent(&host, host.native.as_ref().unwrap()) {
            let native = host.native.as_mut().unwrap();
            deferred.quiescence_callbacks = std::mem::take(&mut native.quiescence_callbacks);
            deferred.termination_callbacks = std::mem::take(&mut native.termination_callbacks);
        }
    }

    // Phase 4 (no SharedHost borrow): flush window refreshes, withdrawals, and user callbacks.
    deferred.flush_post_pump(shared, more, cx);
}

fn schedule_continuation(shared: &SharedHost, host: &mut HostState, cx: &App) {
    let native = host.native.as_mut().unwrap();
    let request = (!native.transport.is_frozen() && native.transport.is_empty())
        .then(|| host.session.native_pointer_continuation())
        .flatten();
    let Some(request) = request else {
        native.wake = None;
        native.blocked_continuation = None;
        return;
    };
    if native.blocked_continuation == Some(request) {
        native.wake = None;
        return;
    }
    native.blocked_continuation = None;
    let capture = native
        .callbacks
        .values()
        .find(|callbacks| callbacks.binding() == request.source_binding().window())
        .and_then(NativeCallbacks::active_capture);
    if native
        .wake
        .as_ref()
        .is_some_and(|wake| wake.request == request && wake.capture == capture)
    {
        return;
    }
    let timer = host
        .clock
        .timer(request.deadline().saturating_sub(host.observed_at()));
    let weak = Rc::downgrade(shared);
    let task = cx.spawn(async move |cx| {
        timer.await;
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let _ = cx.update(|cx| continue_native_pointer(&shared, request, capture, cx));
    });
    host.native.as_mut().unwrap().wake = Some(NativeWake {
        request,
        capture,
        _task: task,
    });
}

fn continue_native_pointer(
    shared: &SharedHost,
    request: dockspace::runtime::NativePointerContinuation,
    capture: Option<super::super::native::NativeCapture>,
    cx: &mut App,
) {
    use dockspace::runtime::{PointerDeviceKind, PointerEventMetadata};

    {
        let mut host = shared.borrow_mut();
        let request_current = host.session.native_pointer_continuation() == Some(request);
        let overdue = host.observed_at() >= request.deadline();
        let Some(native) = host.native.as_mut() else {
            return;
        };
        if !native
            .wake
            .as_ref()
            .is_some_and(|wake| wake.request == request && wake.capture == capture)
        {
            return;
        }
        native.wake = None;
        let transport_ready = native.transport.is_empty() && !native.transport.is_frozen();
        if !request_current {
            return;
        }
        if !overdue {
            // OS timers can wake slightly ahead of the monotonic deadline;
            // re-arm for the remaining duration instead of stranding this wake.
            schedule_continuation(shared, &mut host, cx);
            return;
        }
        // Earlier original input must commit before this finite observation.
        if !transport_ready {
            return;
        }
    }

    let sample = capture.and_then(|capture| {
        cx.sample_native_captured_drag(capture.source_window(), capture.generation())
    });
    let mut host = shared.borrow_mut();
    if host.session.native_pointer_continuation() != Some(request)
        || host.has_unaccepted_update()
        || !host.native.as_ref().is_some_and(|native| {
            !native.transport.is_frozen()
                && native.transport.is_empty()
                && native.callbacks.values().any(|callbacks| {
                    callbacks.binding() == request.source_binding().window()
                        && callbacks.active_capture() == capture
                        && capture.is_some()
                })
        })
    {
        return;
    }
    let input = sample.and_then(|(frame, stack)| {
        host.native.as_ref()?.bridge.as_ref()?.continuation_input(
            &host.session,
            request,
            frame,
            &stack,
            PointerEventMetadata::new(PointerDeviceKind::Mouse, host.observed_at()),
        )
    });
    let native = host.native.as_mut().unwrap();
    // Each unchanged challenge gets one finite attempt. Neither a failed sample
    // nor an accepted continuation can schedule its own overdue timer forever.
    native.blocked_continuation = Some(request);
    if let Some(input) = input {
        native.transport.pointer(input, cx);
    } else {
        // A failed finite sample cannot replay cached coordinates or turn
        // unavailable native-local geometry into a synthetic gesture cancel.
        // Leave the held gesture intact and wait for a real input/new challenge.
        native.reports.push_back(NativeReport::Unsupported {
            operation: "fresh stationary native pointer, hover and capture sampling",
        });
    }
}
