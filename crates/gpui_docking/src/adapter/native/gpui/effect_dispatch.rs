//! Dispatch ordered native effects and settle their exact backend tickets.

use super::*;

pub(in crate::adapter::native) struct PendingDispatch {
    callbacks: NativeCallbacks,
    ticket: PendingTicket,
    _subscription: Subscription,
}

enum PendingTicket {
    Mutation(WindowMutationTicket),
    Focus(WindowActivationTicket),
}

enum PendingOutcome {
    Retired,
    Mutation(WindowMutationOutcome),
    Focus(WindowActivationTerminal),
}

impl PendingDispatch {
    fn outcome(&self, binding: NativeWindowBinding) -> Option<PendingOutcome> {
        if binding != self.callbacks.binding() || !self.callbacks.accepts_pointer_controls() {
            return Some(PendingOutcome::Retired);
        }
        match &self.ticket {
            PendingTicket::Mutation(ticket) => ticket
                .observation()
                .map(|observation| PendingOutcome::Mutation(observation.outcome)),
            PendingTicket::Focus(ticket) => ticket
                .snapshot()
                .status()
                .terminal()
                .map(PendingOutcome::Focus),
        }
    }
}

impl NativeBridge {
    /// Dispatches the exact core FIFO outside `SharedHost` and entity borrows.
    ///
    /// A queued placement or pointer-input mutation retains the original affine
    /// request and blocks dependent work for that window until its real
    /// observation arrives. Other windows and terminal cleanup can progress.
    pub fn dispatch_effects(
        &mut self,
        factory: &mut NativeWindowFactory<'_>,
        cx: &mut App,
    ) -> Result<usize, NativeBridgeError> {
        if self.opening.is_some() {
            self.register_opening()?;
            self.refresh_dependencies(cx)?;
        }
        for (_, _, callbacks) in self.windows.iter() {
            callbacks.observe_open_retirement(cx);
            callbacks.dispatch_capture_cancellations(cx);
        }
        for (_, _, callbacks) in self
            .windows
            .iter()
            .filter(|(_, _, callbacks)| callbacks.0.observations.borrow().has_pending_close())
        {
            self.remove_window(callbacks, cx);
        }
        self.finish_pending(cx);
        let mut dispatched = 0;
        while let Some(request) = self.effects.take_next() {
            let operation = request.operation().clone();
            if (self.draining || self.transport.is_frozen())
                && !matches!(
                    operation,
                    NativeEffectOperation::CompensatingClose { .. }
                        | NativeEffectOperation::ReleaseChild { .. }
                        | NativeEffectOperation::AwaitCleanup { .. }
                        | NativeEffectOperation::ResolveNativeClose {
                            resolution: NativeCloseDisposition::Accept,
                            ..
                        }
                )
            {
                self.transport.push(
                    Inbound::Effect(
                        request.dispatch_failed(NativeDispatchFailure::ProviderStopped),
                    ),
                    cx,
                );
                dispatched += 1;
                continue;
            }
            match operation {
                NativeEffectOperation::CreateWindow {
                    binding,
                    placement,
                    role,
                }
                | NativeEffectOperation::RequestReplacement {
                    binding,
                    placement,
                    role,
                } => {
                    if self.geometry.is_none()
                        || !self
                            .capabilities
                            .supports(NativeHostCapability::GlobalWindowPlacement)
                    {
                        self.transport.push(
                            Inbound::Effect(
                                request.unsupported(NativeUnsupportedReason::BackendUnsupported),
                            ),
                            cx,
                        );
                    } else {
                        let drag_tear_off = request.drag_tear_off();
                        self.create_window(
                            request,
                            NativeWindowRequest {
                                binding,
                                outer_bounds: placement,
                                role,
                                drag_tear_off,
                            },
                            factory,
                            cx,
                        )?;
                    }
                }
                NativeEffectOperation::CompensatingClose { binding }
                | NativeEffectOperation::ReleaseChild { binding } => {
                    self.close_effect(request, binding, cx)
                }
                NativeEffectOperation::ResolveNativeClose {
                    close,
                    resolution: NativeCloseDisposition::Accept,
                } => self.close_effect(request, close.binding(), cx),
                NativeEffectOperation::CancelRootClose { binding } => {
                    self.cancel_close_effect(request, binding, cx)
                }
                NativeEffectOperation::ResolveNativeClose {
                    close,
                    resolution: NativeCloseDisposition::Cancel,
                } => self.cancel_close_effect(request, close.binding(), cx),
                NativeEffectOperation::RetainChild { binding } => {
                    if self.windows.get(binding.window()).is_some() {
                        let _ = request.begin_dispatch().accepted();
                    } else {
                        self.transport.push(
                            Inbound::Effect(
                                request.dispatch_failed(NativeDispatchFailure::WindowUnavailable),
                            ),
                            cx,
                        );
                    }
                }
                NativeEffectOperation::AwaitCleanup { binding } => {
                    if let Some(callbacks) = self.windows.get(binding.window()) {
                        if let Some(NativeEffectAcknowledgement::Cleanup(observation)) =
                            request.begin_dispatch().accepted()
                        {
                            callbacks
                                .0
                                .cleanup
                                .borrow_mut()
                                .push((binding, observation));
                        }
                    } else {
                        self.transport.push(
                            Inbound::Effect(
                                request.dispatch_failed(NativeDispatchFailure::WindowUnavailable),
                            ),
                            cx,
                        );
                    }
                }
                NativeEffectOperation::SetPointerPassthrough { binding, enabled } => {
                    self.pointer_input_effect(request, binding, enabled, cx)
                }
                NativeEffectOperation::RequestFocus { binding } => {
                    self.focus_effect(request, binding, cx)
                }
                _ => self.transport.push(
                    Inbound::Effect(
                        request.unsupported(NativeUnsupportedReason::BackendUnsupported),
                    ),
                    cx,
                ),
            }
            dispatched += 1;
        }
        Ok(dispatched)
    }

    fn create_window(
        &mut self,
        request: NativeEffectRequest,
        open: NativeWindowRequest,
        factory: &mut NativeWindowFactory<'_>,
        cx: &mut App,
    ) -> Result<(), NativeBridgeError> {
        if self.windows.get(open.binding.window()).is_some() {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::AdapterRejected)),
                cx,
            );
            return Ok(());
        }
        let callbacks = NativeCallbacks::new(
            open.binding.window(),
            self.transport.clone(),
            self.geometry.clone(),
        );
        callbacks.0.create_request.set(Some(open));
        self.opening = Some(callbacks.clone());
        let dispatch = request.begin_dispatch();
        match factory(open, &callbacks, cx) {
            Err(error) => {
                let retirement_ticket = error.retirement_ticket.clone();
                if let Some(ticket) = retirement_ticket.as_ref() {
                    callbacks.retain_open_retirement(ticket.clone(), cx);
                }
                let owns_native = retirement_ticket.is_some() || callbacks.window_id().is_some();
                let result = if owns_native {
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost)
                } else {
                    dispatch.not_applied(NativeDispatchFailure::AdapterRejected)
                };
                self.transport.push(Inbound::Effect(result), cx);
                self.reports.push_back(NativeReport::OpenFailed {
                    binding: open.binding,
                    stage: error.stage,
                    message: error.message,
                    retirement_ticket,
                });
                if !owns_native {
                    self.opening = None;
                } else {
                    self.register_opening()?;
                    self.refresh_dependencies(cx)?;
                }
            }
            Ok(handle) => {
                let registered = self
                    .register_opening()
                    .and_then(|()| self.refresh_dependencies(cx));
                if let Err(error) = registered {
                    self.transport.push(
                        Inbound::Effect(
                            dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                        ),
                        cx,
                    );
                    return Err(error);
                }
                if callbacks.handle() != Some(handle) {
                    self.transport.push(
                        Inbound::Effect(
                            dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                        ),
                        cx,
                    );
                    return Err(NativeBridgeError::UnknownBinding);
                }
                let placement = handle.update(cx, |_, window, _| {
                    if let Some(dispatch) = callbacks.0.create_placement.borrow_mut().take() {
                        return Some(dispatch);
                    }
                    self.geometry
                        .as_ref()
                        .and_then(|geometry| (geometry.client_placement)(open.outer_bounds, window))
                        .map(|request| window.request_physical_window_placement(request))
                });
                match placement {
                    Ok(Some(WindowMutationDispatch::Queued(ticket))) => {
                        let wake = self.transport.wake_after_callback(cx);
                        let subscription = ticket.subscribe(move |_| wake());
                        self.await_observation(
                            dispatch,
                            PendingDispatch {
                                callbacks,
                                ticket: PendingTicket::Mutation(ticket),
                                _subscription: subscription,
                            },
                            cx,
                        );
                    }
                    Ok(Some(WindowMutationDispatch::Unchanged))
                        if callbacks.provisional_placement_settled() =>
                    {
                        callbacks.observe_completed_dispatch(dispatch, cx);
                    }
                    Ok(None) | Ok(Some(WindowMutationDispatch::Unsupported)) => {
                        self.reports.push_back(NativeReport::Unsupported {
                            operation: "native outer-frame to client placement conversion",
                        });
                        // The factory already created a native owner. Failure
                        // to place it cannot be reported as not-applied.
                        self.transport.push(
                            Inbound::Effect(
                                dispatch
                                    .indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                            ),
                            cx,
                        );
                    }
                    _ => self.transport.push(
                        Inbound::Effect(
                            dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                        ),
                        cx,
                    ),
                }
            }
        }
        Ok(())
    }

    fn await_observation(
        &mut self,
        dispatch: NativeEffectDispatch,
        pending: PendingDispatch,
        cx: &mut App,
    ) {
        if let Err((dispatch, _)) = self.effects.await_observation(dispatch, pending) {
            self.transport.push(
                Inbound::Effect(
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            );
        }
    }

    pub(super) fn finish_pending(&mut self, cx: &mut App) {
        let completed: Vec<_> = self
            .effects
            .pending()
            .filter_map(|(binding, pending)| {
                pending.outcome(binding).map(|outcome| (binding, outcome))
            })
            .collect();
        for (binding, outcome) in completed {
            let Some((dispatch, pending)) = self.effects.take_pending(binding) else {
                continue;
            };
            let result = match outcome {
                PendingOutcome::Retired => {
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost)
                }
                PendingOutcome::Mutation(outcome) => {
                    let create = matches!(
                        dispatch.operation(),
                        NativeEffectOperation::CreateWindow { .. }
                            | NativeEffectOperation::RequestReplacement { .. }
                    );
                    if matches!(
                        outcome,
                        WindowMutationOutcome::Exact | WindowMutationOutcome::Adjusted
                    ) && pending.callbacks.provisional_placement_settled()
                    {
                        pending.callbacks.observe_completed_dispatch(dispatch, cx);
                        continue;
                    }
                    if create {
                        dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost)
                    } else {
                        match outcome {
                            WindowMutationOutcome::Unsupported => {
                                dispatch.unsupported(NativeUnsupportedReason::BackendUnsupported)
                            }
                            WindowMutationOutcome::Rejected => {
                                dispatch.not_applied(NativeDispatchFailure::AdapterRejected)
                            }
                            _ => dispatch
                                .indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                        }
                    }
                }
                PendingOutcome::Focus(terminal) => {
                    match terminal {
                        WindowActivationTerminal::Activated => {
                            let _ = dispatch.accepted();
                            // Success belongs to this activation. Report current focus independently.
                            self.transport.focused_window(cx.focused_window(), cx);
                            continue;
                        }
                        WindowActivationTerminal::Unsupported => {
                            dispatch.unsupported(NativeUnsupportedReason::BackendUnsupported)
                        }
                        WindowActivationTerminal::Rejected => {
                            dispatch.not_applied(NativeDispatchFailure::AdapterRejected)
                        }
                        _ => dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                    }
                }
            };
            self.transport.push(Inbound::Effect(result), cx);
        }
    }

    fn pointer_input_effect(
        &mut self,
        request: NativeEffectRequest,
        binding: NativeSurfaceBinding,
        enabled: bool,
        cx: &mut App,
    ) {
        let Some(callbacks) = self.windows.get(binding.window()).cloned() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        let Some(handle) = callbacks.handle() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        let dispatch = request.begin_dispatch();
        match handle.update(cx, |_, window, _| window.request_pointer_input(!enabled)) {
            Ok(WindowMutationDispatch::Queued(ticket)) => {
                let wake = self.transport.wake_after_callback(cx);
                let subscription = ticket.subscribe(move |_| wake());
                self.await_observation(
                    dispatch,
                    PendingDispatch {
                        callbacks,
                        ticket: PendingTicket::Mutation(ticket),
                        _subscription: subscription,
                    },
                    cx,
                );
            }
            Ok(WindowMutationDispatch::Unchanged) => {
                callbacks.observe_completed_dispatch(dispatch, cx);
            }
            Ok(WindowMutationDispatch::Unsupported) => self.transport.push(
                Inbound::Effect(dispatch.unsupported(NativeUnsupportedReason::BackendUnsupported)),
                cx,
            ),
            Ok(WindowMutationDispatch::Rejected) => self.transport.push(
                Inbound::Effect(dispatch.not_applied(NativeDispatchFailure::AdapterRejected)),
                cx,
            ),
            _ => self.transport.push(
                Inbound::Effect(
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            ),
        }
    }

    fn focus_effect(
        &mut self,
        request: NativeEffectRequest,
        binding: NativeSurfaceBinding,
        cx: &mut App,
    ) {
        let Some(callbacks) = self.windows.get(binding.window()).cloned() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        let Some(handle) = callbacks.handle() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        if !self
            .capabilities
            .supports(NativeHostCapability::WindowActivationControl)
        {
            self.transport.push(
                Inbound::Effect(request.unsupported(NativeUnsupportedReason::BackendUnsupported)),
                cx,
            );
            return;
        }
        let dispatch = request.begin_dispatch();
        match handle.update(cx, |_, window, _| {
            (window.window_capabilities().activation == PlatformWindowActivationSupport::Observed)
                .then(|| window.activate_window())
        }) {
            Ok(Some(ticket)) => {
                let wake = self.transport.wake_after_callback(cx);
                let subscription = ticket.subscribe(move |_| wake());
                self.await_observation(
                    dispatch,
                    PendingDispatch {
                        callbacks,
                        ticket: PendingTicket::Focus(ticket),
                        _subscription: subscription,
                    },
                    cx,
                );
            }
            Ok(None) => self.transport.push(
                Inbound::Effect(dispatch.unsupported(NativeUnsupportedReason::BackendUnsupported)),
                cx,
            ),
            Err(_) => self.transport.push(
                Inbound::Effect(
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            ),
        }
    }

    fn cancel_close_effect(
        &mut self,
        request: NativeEffectRequest,
        binding: NativeSurfaceBinding,
        cx: &mut App,
    ) {
        let Some(callbacks) = self.windows.get(binding.window()) else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        if callbacks.0.close_sent.get() || callbacks.0.logical_closed.get() {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        }
        if let Some(close) = callbacks.0.observations.borrow_mut().take_pending_close() {
            self.transport.push(
                Inbound::Effect(close.not_applied(NativeDispatchFailure::AdapterRejected)),
                cx,
            );
        }
        if let Some(NativeEffectAcknowledgement::Close(ack)) = request.begin_dispatch().accepted() {
            callbacks.0.close_requested.set(false);
            self.transport.push(
                Inbound::Close(binding.window(), NativeCloseState::Clear, Some(ack)),
                cx,
            );
        }
    }

    fn close_effect(
        &mut self,
        request: NativeEffectRequest,
        binding: NativeSurfaceBinding,
        cx: &mut App,
    ) {
        let Some(callbacks) = self.windows.get(binding.window()).cloned() else {
            self.transport.push(
                Inbound::Effect(request.dispatch_failed(NativeDispatchFailure::WindowUnavailable)),
                cx,
            );
            return;
        };
        if callbacks.0.native_terminal.get()
            || callbacks.0.close_sent.get()
            || callbacks.0.observations.borrow().has_pending_close()
        {
            self.transport.push(
                Inbound::Effect(
                    request.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            );
            return;
        }
        let pending = callbacks
            .0
            .observations
            .borrow_mut()
            .await_destruction(request.begin_dispatch());
        if let Err(dispatch) = pending {
            self.transport.push(
                Inbound::Effect(
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            );
            return;
        }
        self.remove_window(&callbacks, cx);
    }
}
