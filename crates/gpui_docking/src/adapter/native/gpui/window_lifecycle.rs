//! Bind native windows and observe their actual platform and renderer lifecycle.

use super::*;

impl NativeCallbacks {
    pub(in crate::adapter::native) fn new(
        binding: NativeWindowBinding,
        transport: NativeTransport,
        geometry: Option<Rc<NativeGeometry>>,
    ) -> Self {
        Self(Rc::new(CallbackState {
            binding: Cell::new(binding),
            create_request: Cell::new(None),
            window: Cell::new(None),
            transport,
            geometry,
            bounds_observer: RefCell::new(None),
            subscriptions: RefCell::new(Vec::new()),
            facts: Cell::new(None),
            queued_properties: Cell::new(None),
            observations: RefCell::new(NativeWindowObservationOutbox::new(binding)),
            close_requested: Cell::new(false),
            close_sent: Cell::new(false),
            logical_closed: Cell::new(false),
            native_terminal: Cell::new(false),
            sealed: Cell::new(false),
            anchor: Cell::new(false),
            root_detached: Cell::new(false),
            capture: Cell::new(None),
            capture_start: Cell::new(None),
            user_cancelled_capture: Cell::new(None),
            drag_tear_off: Cell::new(None),
            capture_releases: RefCell::new(Vec::new()),
            capture_failed: Cell::new(false),
            capture_cancellations: RefCell::new(VecDeque::new()),
            capture_observation_missing: Cell::new(false),
            capture_ingress: Cell::new(0),
            placement_ready: Cell::new(false),
            cleanup: RefCell::new(Vec::new()),
            open_retirement: RefCell::new(None),
            create_placement: RefCell::new(None),
            provisional_placement: RefCell::new(None),
        }))
    }

    /// Opens an ordinary GPUI-owned native window with early lifecycle interception.
    ///
    /// `build_root` mounts the original retained Views through the caller's surface.
    /// Do not derive `transient_for` from the anchor: it is a lifetime peer window.
    pub fn open<V: Render + 'static>(
        &self,
        options: WindowOptions,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
        cx: &mut App,
    ) -> Result<AnyWindowHandle, NativeOpenFailure> {
        let callbacks = self.clone();
        let window = cx.open_window_detailed(options, move |window, cx| {
            if callbacks.install(window, cx).is_err() {
                window.remove_window(cx);
            }
            build_root(window, cx)
        })?;
        Ok(window.into())
    }

    /// Installs before initial drawing for a window whose OS lifetime GPUI owns.
    ///
    /// Application-owned external attachments require their owning adapter to
    /// distinguish binding detachment from OS destruction; this entry point is
    /// deliberately for ordinary GPUI-created native windows only.
    pub fn install(&self, window: &mut Window, cx: &mut App) -> Result<(), NativeBridgeError> {
        let handle = window.window_handle();
        if let Some(existing) = self.0.window.get() {
            return if existing.window_id() == handle.window_id() {
                Ok(())
            } else {
                Err(NativeBridgeError::WindowAlreadyBound)
            };
        }
        if !window.presentation_facts().native_created {
            return Err(NativeBridgeError::UnknownBinding);
        }
        self.0.window.set(Some(handle));
        self.0.facts.set(Some(NativeWindowFacts::live()));
        let weak = Rc::downgrade(&self.0);
        window.on_window_should_close(cx, move |_, cx| {
            let Some(state) = weak.upgrade() else {
                return true;
            };
            if state.sealed.get() || state.root_detached.get() {
                return true;
            }
            if state.anchor.get() {
                state.transport.freeze(cx);
            } else if !state.close_requested.replace(true) {
                state.transport.push(
                    Inbound::Close(state.binding.get(), NativeCloseState::Requested, None),
                    cx,
                );
            }
            false
        });
        let weak = Rc::downgrade(&self.0);
        let logical = cx.on_window_closed(move |cx, window_id| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if state.window.get().map(|handle| handle.window_id()) != Some(window_id)
                || state.sealed.get()
            {
                return;
            }
            state.logical_closed.set(true);
            if let Some(ticket) = cx.native_window_retirement_ticket(window_id) {
                *state.open_retirement.borrow_mut() = Some(ticket);
            }
            if state.anchor.get() {
                state.transport.freeze(cx);
            }
            state
                .transport
                .push(Inbound::LogicalClosed(state.binding.get()), cx);
        });
        let weak = Rc::downgrade(&self.0);
        let terminal = cx.on_window_native_terminal(move |cx, window_id| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if state.window.get().map(|handle| handle.window_id()) != Some(window_id)
                || state.sealed.get()
            {
                return;
            }
            state.native_terminal.set(true);
            state.capture.set(None);
            state.capture_start.set(None);
            state.user_cancelled_capture.set(None);
            state.capture_releases.borrow_mut().clear();
            state.capture_failed.set(false);
            state.capture_cancellations.borrow_mut().clear();
            if state.anchor.get() {
                state.transport.freeze(cx);
            }
            let facts = state.observations.borrow().destroyed();
            state
                .transport
                .push(Inbound::NativeTerminal(state.binding.get(), facts), cx);
        });
        let weak = Rc::downgrade(&self.0);
        let initial = window.observe_window_initial_presentation(move |window, cx| {
            if let Some(state) = weak.upgrade() {
                NativeCallbacks(state).observe(window, cx);
            }
        });
        let weak = Rc::downgrade(&self.0);
        let focus = Rc::downgrade(&self.0);
        let bounds = cx.new(|cx| {
            let bounds = cx.observe_window_bounds(window, move |_, window, cx| {
                if let Some(state) = weak.upgrade() {
                    NativeCallbacks(state).observe(window, cx);
                }
            });
            let focus = cx.observe_window_activation(window, move |_, _, cx| {
                if let Some(state) = focus.upgrade()
                    && !state.sealed.get()
                    && !state.logical_closed.get()
                {
                    NativeCallbacks(state).schedule_focus_observation(cx);
                }
            });
            vec![bounds, focus]
        });
        self.0.bounds_observer.replace(Some(bounds));
        let weak = Rc::downgrade(&self.0);
        let display = window.observe_window_display(move |window, cx| {
            if let Some(state) = weak.upgrade() {
                NativeCallbacks(state).observe(window, cx);
            }
        });
        let weak = Rc::downgrade(&self.0);
        let visibility = window.observe_window_visibility(move |_, window, cx| {
            if let Some(state) = weak.upgrade() {
                NativeCallbacks(state).observe(window, cx);
            }
        });
        self.0
            .subscriptions
            .borrow_mut()
            .extend([logical, terminal, initial, display, visibility]);
        self.observe(window, cx);
        self.schedule_focus_observation(cx);
        Ok(())
    }

    fn schedule_focus_observation(&self, cx: &mut App) {
        let weak = Rc::downgrade(&self.0);
        // Installation and native activation are hints. Sample after the current
        // construction transaction has published its initial window inventory.
        cx.defer(move |cx| {
            if let Some(state) = weak.upgrade()
                && !state.sealed.get()
                && !state.logical_closed.get()
            {
                state.transport.focused_window(cx.focused_window(), cx);
            }
        });
    }

    /// Captures current platform facts. Call from bounds/DPI/visibility observations.
    ///
    /// Renderer preparation uses actual native visibility, positive client extent
    /// and GPUI's completed initial presentation. First-live still needs U5's exact
    /// semantic output and original renderer submission ticket.
    pub fn observe(&self, window: &Window, cx: &mut App) {
        if self.0.sealed.get()
            || self.0.logical_closed.get()
            || self.window_id() != Some(window.window_handle().window_id())
        {
            return;
        }
        if let Some(facts) = self.window_facts(window, || None) {
            self.publish_observation(facts, cx);
        }
    }

    fn publish_observation(&self, facts: NativeWindowFacts, cx: &mut App) {
        // Redraws can observe the same properties while another window holds
        // the update lease. Deduplicate the queued samples without treating
        // their receipts as confirmed or suppressing a later change back.
        let properties = facts.without_effect_acknowledgements();
        let changed = self.0.queued_properties.get().map_or_else(
            || self.0.observations.borrow().observe(facts).is_some(),
            |queued| facts != properties || queued != properties,
        );
        if changed {
            self.0.queued_properties.set(Some(properties));
            self.0
                .transport
                .push(Inbound::Observed(self.binding(), facts), cx);
        }
    }

    pub(super) fn observe_completed_dispatch(&self, dispatch: NativeEffectDispatch, cx: &mut App) {
        let mut dispatch = Some(dispatch);
        if let Some(handle) = self.handle() {
            let _ = handle.update(cx, |_, window, cx| {
                if self.0.logical_closed.get() || self.0.native_terminal.get() {
                    return;
                }
                if let Some(NativeEffectOperation::SetPointerPassthrough { enabled, .. }) =
                    dispatch.as_ref().map(NativeEffectDispatch::operation)
                    && window.platform_facts().accepts_pointer_input == *enabled
                {
                    return;
                }
                let facts = self.window_facts(window, || {
                    let ack = dispatch.take().and_then(NativeEffectDispatch::accepted);
                    if matches!(ack, Some(NativeEffectAcknowledgement::Create(_))) {
                        self.allow_preparation();
                    }
                    ack
                });
                if let Some(facts) = facts {
                    self.publish_observation(facts, cx);
                }
            });
        }
        if let Some(dispatch) = dispatch {
            self.0.transport.push(
                Inbound::Effect(
                    dispatch.indeterminate(NativeIndeterminateReason::AcknowledgementLost),
                ),
                cx,
            );
        }
    }

    /// Observes U5's existing ticket, preserving its real window and frame authority.
    pub fn watch_submission(
        &self,
        ticket: &WindowFrameSubmissionTicket,
        window: &mut Window,
    ) -> Result<(), NativeBridgeError> {
        if self.window_id() != Some(ticket.snapshot().window_id()) || self.0.sealed.get() {
            return Err(NativeBridgeError::UnknownBinding);
        }
        let binding = self.binding();
        let weak = Rc::downgrade(&self.0);
        window.on_exact_frame_submission_presented(ticket, move |snapshot, _, cx| {
            if let Some(state) = weak.upgrade()
                && !state.sealed.get()
            {
                state.transport.push(
                    Inbound::Submission(binding, snapshot, CallbackCustody(weak)),
                    cx,
                );
            }
        });
        Ok(())
    }

    /// Stops future delivery only after the actual native terminal and capture boundary.
    /// U5 must separately retire its original submission callbacks and output owners.
    pub fn seal(&self) -> Result<(), NativeBridgeError> {
        if !self.0.native_terminal.get()
            || !self.0.capture_releases.borrow().is_empty()
            || self.0.capture_failed.get()
            || self
                .0
                .open_retirement
                .borrow()
                .as_ref()
                .is_some_and(|ticket| !ticket.snapshot().quiesced())
        {
            return Err(NativeBridgeError::NotQuiescent);
        }
        self.0.sealed.set(true);
        self.0.bounds_observer.borrow_mut().take();
        self.0.subscriptions.borrow_mut().clear();
        Ok(())
    }

    /// Returns the exact current callback route, never a surface-id lookup.
    pub fn binding(&self) -> NativeWindowBinding {
        self.0.binding.get()
    }

    pub(in crate::adapter) fn accepts_pointer_controls(&self) -> bool {
        !self.0.sealed.get() && !self.0.logical_closed.get() && !self.0.native_terminal.get()
    }
    pub(in crate::adapter) fn has_callback_custody(&self, original: &CallbackCustody) -> bool {
        Rc::downgrade(&self.0).ptr_eq(&original.0)
    }
    /// Returns the full GPUI window id reserved during root construction.
    pub fn window_id(&self) -> Option<open_gpui::WindowId> {
        self.0
            .window
            .get()
            .map(|window| window.window_id())
            .or_else(|| {
                self.0
                    .open_retirement
                    .borrow()
                    .as_ref()
                    .map(|ticket| ticket.snapshot().window_id())
            })
    }

    /// Returns the original core request sent to this lifetime's native factory.
    ///
    /// Enrolled application roots have no create request. The frozen outer intent
    /// remains distinct from converted client bounds and later native observations.
    pub fn create_request(&self) -> Option<NativeWindowRequest> {
        self.0.create_request.get()
    }

    pub(super) fn is_child_window(&self) -> bool {
        self.create_request()
            .is_some_and(|request| request.role == NativeSurfaceRole::Child)
    }

    /// Reads the original placement receipt when a provisional Window was adopted.
    ///
    /// Requested client bounds, target display and release anchor come from the
    /// original request. Native observations and settlement remain separate facts;
    /// this snapshot does not acknowledge creation or first-live presentation.
    pub fn provisional_placement(&self) -> Option<WindowProvisionalPlacementSnapshot> {
        self.0
            .provisional_placement
            .borrow()
            .as_ref()
            .map(|ticket| ticket.snapshot())
    }

    pub(crate) fn retain_open_retirement(
        &self,
        ticket: WindowPresentationShutdownTicket,
        cx: &mut App,
    ) {
        // Even a failure before root construction can own a real native window.
        // Keep GPUI's original shutdown authority; never mint a receipt for it.
        let owner = ticket.snapshot().window_id();
        *self.0.open_retirement.borrow_mut() = Some(ticket);
        if self.0.facts.get().is_none() {
            self.0.facts.set(Some(NativeWindowFacts::live()));
        }
        if self.0.window.get().is_none() {
            let weak = Rc::downgrade(&self.0);
            let subscription = cx.on_window_native_terminal(move |cx, terminal| {
                if terminal == owner
                    && let Some(state) = weak.upgrade()
                {
                    NativeCallbacks(state).observe_open_retirement(cx);
                }
            });
            self.0.subscriptions.borrow_mut().push(subscription);
        }
        self.observe_open_retirement(cx);
    }

    pub(super) fn observe_open_retirement(&self, cx: &mut App) {
        let snapshot = self
            .0
            .open_retirement
            .borrow()
            .as_ref()
            .map(WindowPresentationShutdownTicket::snapshot);
        if self.0.window.get().is_none()
            && snapshot.is_some_and(|snapshot| snapshot.native_terminal())
            && !self.0.native_terminal.replace(true)
        {
            let facts = self.0.observations.borrow().destroyed();
            self.0
                .transport
                .push(Inbound::NativeTerminal(self.binding(), facts), cx);
        }
    }

    pub(in crate::adapter::native) fn correlate_cleanup(
        &self,
        mut result: dockspace::runtime::NativeEffectResult,
    ) -> dockspace::runtime::NativeEffectResult {
        let mut observations = std::mem::take(&mut *self.0.cleanup.borrow_mut()).into_iter();
        let mut unmatched = Vec::new();
        while let Some((binding, observation)) = observations.next() {
            match observation.correlate(result) {
                Ok(correlated) => {
                    result = correlated;
                    unmatched.extend(observations);
                    break;
                }
                Err(error) => {
                    let (observation, predecessor) = error.into_parts();
                    unmatched.push((binding, observation));
                    result = predecessor;
                }
            }
        }
        self.0.cleanup.borrow_mut().extend(unmatched);
        result
    }

    pub(in crate::adapter::native) fn handle(&self) -> Option<AnyWindowHandle> {
        self.0.window.get()
    }
    pub(in crate::adapter::native) fn facts(&self) -> Option<NativeWindowFacts> {
        self.0.facts.get()
    }
    pub(in crate::adapter::native) fn set_facts(&self, facts: NativeWindowFacts) {
        self.0
            .facts
            .set(Some(facts.without_effect_acknowledgements()));
    }

    pub(in crate::adapter::native) fn confirm_observation(&self, facts: NativeWindowFacts) {
        self.0
            .observations
            .borrow_mut()
            .confirm_observation(self.binding(), facts);
        self.set_facts(facts);
    }
    pub(in crate::adapter::native) fn set_logical_closed(&self) {
        self.0.logical_closed.set(true);
    }
    pub(in crate::adapter::native) fn set_native_terminal(&self) {
        self.0.native_terminal.set(true);
    }
    pub(in crate::adapter::native) fn set_anchor(&self) {
        self.0.anchor.set(true);
    }
    pub(in crate::adapter::native) fn allow_preparation(&self) {
        self.0.placement_ready.set(true);
    }
    pub(crate) fn retain_provisional_placement(
        &self,
        dispatch: WindowMutationDispatch,
        ticket: WindowProvisionalPlacementTicket,
    ) {
        self.0.create_placement.replace(Some(dispatch));
        self.0.provisional_placement.replace(Some(ticket));
    }

    pub(super) fn provisional_placement_settled(&self) -> bool {
        self.0
            .provisional_placement
            .borrow()
            .as_ref()
            .is_none_or(|ticket| {
                let snapshot = ticket.snapshot();
                snapshot.outcome() == WindowProvisionalPlacementOutcome::Settled
                    && snapshot
                        .native_facts()
                        .is_some_and(|facts| facts.accepts_placement())
            })
    }
    pub(in crate::adapter::native) fn continue_root(&self, binding: NativeWindowBinding) {
        let previous = self.0.binding.replace(binding);
        self.0.transport.continue_mouse_press(previous, binding);
        // Continuation retains this exact physical window. Its independent
        // geometry and effect proofs must refresh, but inventory remains known.
        self.0.facts.set(Some(if self.0.native_terminal.get() {
            NativeWindowFacts::destroyed()
        } else {
            NativeWindowFacts::live()
        }));
        self.0.observations.borrow_mut().reset_for_binding(binding);
        self.0.queued_properties.set(None);
    }

    pub(super) fn window_facts(
        &self,
        window: &Window,
        acknowledge: impl FnOnce() -> Option<NativeEffectAcknowledgement>,
    ) -> Option<NativeWindowFacts> {
        let platform = window.platform_facts();
        let presentation = window.presentation_facts();
        if !presentation.native_created {
            return None;
        }
        let native_scale = ScaleFactor::new(f64::from(platform.scale_factor)).ok();
        let presentation_scale = ScaleFactor::new(f64::from(window.scale_factor())).ok();
        let client = platform
            .physical_geometry
            .and_then(|geometry| {
                crate::adapter::native::input::physical_rect(geometry.client_bounds())
            })
            .map(NativeWindowClientGeometry::Bounds)
            .or_else(|| {
                let scale = native_scale?.get();
                PhysicalSize::new(
                    f64::from(f32::from(platform.content_size.width)) * scale,
                    f64::from(f32::from(platform.content_size.height)) * scale,
                )
                .ok()
                .map(NativeWindowClientGeometry::Size)
            });
        let size = client.map(|client| match client {
            NativeWindowClientGeometry::Size(size) => size,
            NativeWindowClientGeometry::Bounds(bounds) => bounds.size(),
        });
        let acknowledgement = acknowledge();
        // Use native visibility. Compositor occlusion does not retire a
        // shown window that still owns the captured drag.
        let visible = presentation.native_visible && !platform.is_minimized;
        let local_ready =
            visible && size.is_some_and(|size| size.width() > 0.0 && size.height() > 0.0);
        let prepared = self.0.placement_ready.get()
            && local_ready
            && presentation.initial_presentation == WindowInitialPresentationStatus::Completed;
        let facts = compile_native_window_facts(NativeWindowFactsSample {
            client,
            outer_bounds: self
                .0
                .geometry
                .as_ref()
                .and_then(|geometry| (geometry.outer_bounds)(platform)),
            native_scale,
            presentation_scale,
            visible: Some(presentation.native_visible),
            minimized: Some(platform.is_minimized),
        });
        Some(
            facts
                .with_local_readiness(if local_ready {
                    NativeLocalReadiness::Ready
                } else {
                    NativeLocalReadiness::Unavailable
                })
                .with_input(
                    if platform.accepts_pointer_input {
                        NativeWindowInputState::ReceivesInput
                    } else {
                        NativeWindowInputState::PassThrough
                    },
                    match acknowledgement {
                        Some(NativeEffectAcknowledgement::Input(ack)) => Some(ack),
                        _ => None,
                    },
                )
                .with_create_readiness(
                    if prepared {
                        NativeCreateReadiness::Prepared
                    } else {
                        NativeCreateReadiness::Created
                    },
                    match acknowledgement {
                        Some(NativeEffectAcknowledgement::Create(ack)) => Some(ack),
                        _ => None,
                    },
                )
                .with_close(
                    if self.0.close_requested.get() {
                        NativeCloseState::Requested
                    } else {
                        NativeCloseState::Clear
                    },
                    None,
                ),
        )
    }
}

impl NativeBridge {
    /// Applies the committed placement roster after physical Root continuations.
    pub(in crate::adapter::native) fn update_root_close_authority(
        &self,
        placements: &[NativeSurfaceBinding],
    ) {
        for (binding, _, callbacks) in self.windows.iter() {
            callbacks.0.root_detached.set(
                !callbacks.0.anchor.get()
                    && !callbacks.is_child_window()
                    && !placements
                        .iter()
                        .any(|placement| placement.window() == binding),
            );
        }
    }

    pub(crate) fn preflight_root_binding(
        &self,
        window: &Window,
        anchor: bool,
        cx: &App,
    ) -> Result<(), NativeBridgeError> {
        if self.draining || self.transport.is_frozen() {
            return Err(NativeBridgeError::Draining);
        }
        if self.opening.is_some() {
            return Err(NativeBridgeError::NotQuiescent);
        }
        let owner = window.window_handle().window_id();
        if (anchor && self.anchor.is_some()) || self.binding_for_window(owner).is_some() {
            return Err(NativeBridgeError::WindowAlreadyBound);
        }
        if !window.presentation_facts().native_created {
            return Err(NativeBridgeError::UnknownBinding);
        }
        let anchor = if anchor {
            Some(owner)
        } else {
            self.anchor
                .and_then(|binding| self.windows.get(binding))
                .and_then(NativeCallbacks::window_id)
        };
        if let Some(anchor) = anchor {
            let dependencies = self
                .windows
                .iter()
                .map(|(_, window, _)| window)
                .chain(std::iter::once(owner))
                .filter(|dependency| *dependency != anchor);
            cx.register_native_window_retirement_dependencies(anchor, dependencies)
                .map_err(NativeBridgeError::Dependencies)?;
        }
        Ok(())
    }

    pub(crate) fn retain_root_rollback(
        &mut self,
        binding: NativeWindowBinding,
        ticket: WindowPresentationShutdownTicket,
        anchor: bool,
        cx: &mut App,
    ) -> Result<NativeCallbacks, NativeBridgeError> {
        if self.opening.is_some() {
            return Err(NativeBridgeError::NotQuiescent);
        }
        let owner = ticket.snapshot().window_id();
        let callbacks = if let Some(callbacks) = self.windows.get(binding) {
            if callbacks.window_id() != Some(owner) {
                return Err(NativeBridgeError::WindowAlreadyBound);
            }
            callbacks.clone()
        } else {
            let callbacks =
                NativeCallbacks::new(binding, self.transport.clone(), self.geometry.clone());
            self.windows
                .insert(binding, owner, callbacks.clone())
                .map_err(|_| NativeBridgeError::WindowAlreadyBound)?;
            callbacks
        };
        if anchor && self.anchor.is_none() {
            callbacks.set_anchor();
            self.anchor = Some(binding);
        }
        callbacks.retain_open_retirement(ticket, cx);
        self.refresh_dependencies(cx)?;
        Ok(callbacks)
    }

    pub(crate) fn retain_root_retirement_dependencies(
        &self,
        retirements: impl IntoIterator<Item = open_gpui::WindowId>,
        cx: &App,
    ) -> Result<(), NativeBridgeError> {
        if let Some(anchor) = self
            .anchor
            .and_then(|binding| self.windows.get(binding))
            .and_then(NativeCallbacks::window_id)
        {
            cx.register_native_window_retirement_dependencies(anchor, retirements)
                .map_err(NativeBridgeError::Dependencies)?;
        }
        Ok(())
    }

    pub(super) fn remove_window(&self, callbacks: &NativeCallbacks, cx: &mut App) {
        if callbacks.0.logical_closed.get() || callbacks.0.native_terminal.get() {
            return;
        }
        callbacks.cancel_capture(cx);
        // Only collection of the original release terminal clears this custody.
        // Native destruction cannot substitute for proving capture/route cleanup.
        if callbacks.0.capture.get().is_some()
            || callbacks.0.capture_start.get().is_some()
            || !callbacks.0.capture_releases.borrow().is_empty()
            || !callbacks.0.capture_cancellations.borrow().is_empty()
            || callbacks.0.capture_failed.get()
        {
            return;
        }
        if let Some(handle) = callbacks.handle() {
            if let Ok(ticket) =
                handle.update(cx, |_, window, cx| window.remove_window_with_retirement(cx))
            {
                callbacks.0.close_sent.set(true);
                callbacks.retain_open_retirement(ticket, cx);
            }
        }
    }

    pub(in crate::adapter::native) fn refresh_dependencies(
        &self,
        cx: &App,
    ) -> Result<(), NativeBridgeError> {
        let children: Vec<_> = self
            .windows
            .iter()
            .filter(|(_, _, callbacks)| callbacks.is_child_window())
            .map(|(_, owner, _)| owner)
            .collect();
        if !children.is_empty() {
            // Any application root may be a child's recovery host, independently
            // of the workspace's final retirement anchor. Keep those native
            // owners behind the same real child terminals during App shutdown.
            for (binding, owner, callbacks) in self.windows.iter() {
                if Some(binding) != self.anchor
                    && !callbacks.is_child_window()
                    && callbacks.handle().is_some()
                    && !callbacks.0.logical_closed.get()
                    && !callbacks.0.native_terminal.get()
                {
                    cx.register_native_window_retirement_dependencies(
                        owner,
                        children.iter().copied(),
                    )
                    .map_err(NativeBridgeError::Dependencies)?;
                }
            }
        }
        if let Some(anchor) = self
            .anchor
            .and_then(|binding| self.windows.get(binding))
            .and_then(NativeCallbacks::window_id)
        {
            let dependents = self
                .windows
                .iter()
                .filter(|(binding, _, _)| Some(*binding) != self.anchor)
                .map(|(_, window, _)| window);
            cx.register_native_window_retirement_dependencies(anchor, dependents)
                .map_err(NativeBridgeError::Dependencies)?;
        }
        Ok(())
    }

    /// Freezes this provider and starts forced dependent-first retirement.
    pub fn begin_shutdown(&mut self, cx: &mut App) -> Result<(), NativeBridgeError> {
        self.transport.freeze(cx);
        self.draining = true;
        self.drive_shutdown(cx)
    }

    /// Retries close intents; a failed handle update never becomes a native terminal.
    ///
    /// Child bindings must reach permanent quiescence and leave this bridge before
    /// application roots retire, preserving any non-anchor recovery hosts.
    pub fn drive_shutdown(&mut self, cx: &mut App) -> Result<(), NativeBridgeError> {
        if !self.draining && !self.transport.is_frozen() {
            return Ok(());
        }
        self.draining = true;
        let children_pending = self.opening.is_some()
            || self
                .windows
                .iter()
                .any(|(_, _, callbacks)| callbacks.is_child_window());
        if children_pending {
            for (_, _, callbacks) in self
                .windows
                .iter()
                .filter(|(_, _, callbacks)| callbacks.is_child_window())
            {
                self.remove_window(callbacks, cx);
            }
            // Native terminal alone is insufficient: collection must commit the
            // loss/recovery and permanent binding quiescence, removing the child
            // from this index before any application recovery root is closed.
            return Ok(());
        }
        for (binding, _, callbacks) in self.windows.iter() {
            if Some(binding) != self.anchor {
                self.remove_window(callbacks, cx);
            }
        }
        let dependents_terminal = self
            .windows
            .iter()
            .filter(|(binding, _, _)| Some(*binding) != self.anchor)
            .all(|(_, _, callbacks)| callbacks.0.native_terminal.get());
        if dependents_terminal
            && self.opening.is_none()
            && let Some(anchor) = self.anchor.and_then(|binding| self.windows.get(binding))
        {
            self.remove_window(anchor, cx);
        }
        Ok(())
    }

    /// Declares the caller's exact retired placement permanently free of U5 owners.
    ///
    /// The caller first retires output tickets, artifacts, routes and renderer
    /// sidecars. For a destroyed window, seal its callbacks after true terminal.
    /// A detached placement can skip native terminal only while another current
    /// placement retains the same physical window.
    /// Core ingress must commit before this permanent boundary can be recorded.
    pub fn quiesce_binding(
        &mut self,
        session: &mut DockspaceSession,
        binding: NativeSurfaceBinding,
    ) -> Result<(), NativeBridgeError> {
        if !self.transport.is_empty()
            || !self.transport.callbacks_idle()
            || self.opening.is_some()
            || !self.reports.is_empty()
            || session.has_pending_native_input()
            || self
                .effects
                .queued()
                .any(|request| request.operation().binding().window() == binding.window())
            || self
                .effects
                .pending()
                .any(|(window, _)| window.same_window_lifetime(binding.window()))
            || self.windows.iter().any(|(_, _, callbacks)| {
                callbacks
                    .0
                    .capture_releases
                    .borrow()
                    .iter()
                    .any(|(owner, _)| *owner == binding.window())
            })
        {
            return Err(NativeBridgeError::NotQuiescent);
        }
        if let Some(callbacks) = self.windows.get(binding.window()) {
            let has_successor = self.retired_placements.contains(&binding)
                && session
                    .native_surface_bindings_for_window(binding.window())
                    .next()
                    .is_some();
            if !has_successor && (!callbacks.0.sealed.get() || !callbacks.0.native_terminal.get()) {
                return Err(NativeBridgeError::NotQuiescent);
            }
        }
        session.report_native_binding_quiescence(binding)?;
        if let Some(callbacks) = self.windows.get(binding.window()) {
            callbacks
                .0
                .cleanup
                .borrow_mut()
                .retain(|(owner, _)| *owner != binding);
        }
        self.retired_placements.remove(&binding);
        if session
            .retained_native_surface_bindings_for_window(binding.window())
            .next()
            .is_none()
        {
            let _ = self.windows.remove(binding.window());
        }
        Ok(())
    }
}
