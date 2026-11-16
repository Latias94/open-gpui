//! GPUI owns preview windows until the original core create effect adopts them.
//!
//! A preview is a visual shell, not a second semantic surface or native producer.
//! Only the create effect installs core callbacks. Its destination output shares
//! the provisional semantics ticket with the ordinary presentation pipeline.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use anyhow::{Context as _, Result, anyhow, ensure};
use dockspace::{
    geometry::PhysicalRect,
    model::SurfaceId,
    runtime::{NativeDragTearOff, NativeSurfaceBinding},
};
use open_gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, IntoElement,
    PlatformNativePointerPhysicalFrame, Subscription, Window, WindowFrameSubmissionOutcome,
    WindowFrameSubmissionTicket, WindowId, WindowInitialPresentationStatus, WindowOptions,
    WindowPhysicalPlacementRequest, WindowPresentationShutdownTicket,
    WindowProvisionalPlacementOutcome, WindowProvisionalPlacementPurpose,
    WindowProvisionalPlacementRequest, WindowProvisionalPlacementTicket,
    WindowProvisionalRevealOutcome, WindowProvisionalRevealTicket,
    WindowProvisionalSemanticsTicket, WindowProvisionalSession, WindowProvisionalSessionPhase, div,
    prelude::*,
};

use super::{
    DockSurface, HostState, NativeBridge, NativeCallbacks, NativeGeometry, NativeOpenFailure,
    NativeRoot, NativeWindowRequest, SharedHost,
};
use crate::adapter::element::Packet;
use crate::adapter::native::NativeCapture;

#[cfg(test)]
#[path = "provisional_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Preview {
    pub(crate) host_surface: SurfaceId,
    pub(crate) target_surface: SurfaceId,
    pub(crate) placement: PhysicalRect,
}

#[derive(Clone, Copy)]
struct AcceptedPreview {
    owner: WindowId,
    drag: NativeDragTearOff,
    visual: Preview,
    route: Option<PlatformNativePointerPhysicalFrame>,
    capture: Option<NativeCapture>,
}

#[derive(Default)]
pub(super) struct Provisional {
    preview: Option<AcceptedPreview>,
    opening: Option<Rc<RefCell<ProvisionalWindow>>>,
    promotions: BTreeMap<WindowId, Rc<RefCell<ProvisionalWindow>>>,
    retirements: BTreeMap<WindowId, WindowPresentationShutdownTicket>,
    awaiting_retirement: BTreeMap<WindowId, Rc<RefCell<ProvisionalWindow>>>,
    dependencies: BTreeSet<WindowId>,
    failed: Option<SurfaceId>,
    generation: u64,
}

pub(super) struct ProvisionalWindow {
    window: Option<AnyWindowHandle>,
    drag: NativeDragTearOff,
    preview: Preview,
    session: WindowProvisionalSession,
    reveal: Option<WindowProvisionalRevealTicket>,
    placement: Option<WindowProvisionalPlacementTicket>,
    semantics: Option<WindowProvisionalSemanticsTicket>,
    binding: Option<NativeSurfaceBinding>,
    admitted: bool,
    failure: Option<String>,
    route: PlatformNativePointerPhysicalFrame,
    capture: Option<NativeCapture>,
    release_frame: Option<PlatformNativePointerPhysicalFrame>,
    peers: Vec<WindowId>,
    placement_generation: u64,
    _initial: Option<Subscription>,
}

impl Provisional {
    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn placement_for_test(
        &self,
        owner: WindowId,
    ) -> Option<(PhysicalRect, open_gpui::WindowProvisionalPlacementSnapshot)> {
        let state = self.opening.as_ref()?.borrow();
        if state.window?.window_id() != owner || state.binding.is_some() {
            return None;
        }
        Some((
            state.preview.placement,
            state.placement.as_ref()?.snapshot(),
        ))
    }

    pub(super) fn inspection(&self) -> (bool, usize, usize) {
        (
            self.opening.is_some(),
            self.promotions.len(),
            self.retirements.len() + self.awaiting_retirement.len(),
        )
    }

    pub(super) fn is_quiescent(&self) -> bool {
        self.opening.is_none()
            && self.promotions.is_empty()
            && self.retirements.is_empty()
            && self.awaiting_retirement.is_empty()
    }
}

pub(crate) fn accept_preview(
    host: &mut HostState,
    owner: WindowId,
    packets: &BTreeMap<SurfaceId, Rc<Packet>>,
) {
    let drag = host.session.native_drag_tear_off();
    let Some(native) = &mut host.native else {
        return;
    };
    let preview = packets.values().find_map(|packet| packet.native_preview);
    if let Some((preview, drag)) = preview.zip(drag) {
        native.provisional.preview = Some(AcceptedPreview {
            owner,
            drag,
            visual: preview,
            route: native.bridge.as_ref().and_then(NativeBridge::pointer_frame),
            capture: native
                .bridge
                .as_ref()
                .and_then(NativeBridge::pointer_capture),
        });
    } else if native
        .provisional
        .preview
        .is_some_and(|preview| preview.owner == owner)
    {
        native.provisional.preview = None;
    }
}

pub(super) fn observe_release(
    native: &mut super::ManagedHost,
    capture: NativeCapture,
    frame: Option<PlatformNativePointerPhysicalFrame>,
) {
    if let Some(state) = &native.provisional.opening {
        let mut state = state.borrow_mut();
        if state.capture == Some(capture) {
            state.release_frame = frame;
        }
    }
    if native
        .provisional
        .preview
        .is_some_and(|preview| preview.capture == Some(capture))
    {
        native.provisional.preview = None;
    }
}

pub(super) fn observe_admitted(
    native: &mut super::ManagedHost,
    binding: NativeSurfaceBinding,
    cx: &mut App,
) {
    for state in native.provisional.promotions.values() {
        let mut state = state.borrow_mut();
        if state.binding == Some(binding) {
            state.admitted = true;
            native.transport.wake(cx);
        }
    }
}

fn physical_request(
    outer: PhysicalRect,
    route: PlatformNativePointerPhysicalFrame,
    window: &Window,
    cx: &App,
) -> Result<WindowPhysicalPlacementRequest> {
    let display = route
        .target_display()
        .context("preview has no original target display")?;
    ensure!(
        cx.display_snapshot()
            .physical_displays()
            .is_some_and(|roster| roster.contains(&display)),
        "preview target display observation is no longer current"
    );
    let client = NativeGeometry::gpui_client_placement(outer, window)
        .context("preview outer frame has no target-DPI client conversion")?;
    ensure!(
        client.target_display() == Some(display),
        "preview conversion selected a different target display"
    );
    WindowPhysicalPlacementRequest::try_new_for_display(
        client.client_bounds(),
        route.global_position(),
        display,
    )
    .context("preview placement cannot bind its original pointer and target display")
}

fn placement_request(
    state: &mut ProvisionalWindow,
    outer: PhysicalRect,
    route: PlatformNativePointerPhysicalFrame,
    purpose: WindowProvisionalPlacementPurpose,
    window: &Window,
    cx: &App,
) -> Result<WindowProvisionalPlacementRequest> {
    let physical = physical_request(outer, route, window, cx)?;
    state.placement_generation = state
        .placement_generation
        .checked_add(1)
        .context("preview placement generation exhausted")?;
    WindowProvisionalPlacementRequest::try_new(
        state.placement_generation,
        purpose,
        physical.client_bounds(),
        route.global_position(),
        route.target_display().context("missing preview display")?,
    )
    .context("invalid provisional placement request")
}

fn wake_after_frame(shared: &SharedHost, window: &Window) {
    let weak = Rc::downgrade(shared);
    window.on_next_frame(move |_, cx| {
        if let Some(shared) = weak.upgrade() {
            super::schedule(&shared, cx);
        }
    });
}

/// Runs within this window's normal render; the shell never mounts a reserved core surface.
pub(super) fn prepare(
    dock: &DockSurface,
    state: &Rc<RefCell<ProvisionalWindow>>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let mut state = state.borrow_mut();
    if state.session.snapshot().phase() == WindowProvisionalSessionPhase::Promoted {
        return None;
    }
    advance_presentation(&mut state, &dock.host, window, cx);
    if state.semantics.is_some() {
        return None;
    }
    wake_after_frame(&dock.host, window);
    let colors = dock.host.borrow().config.colors.clone();
    Some(
        div()
            .size_full()
            .bg(colors.preview)
            .border_1()
            .border_color(colors.accent)
            .into_any_element(),
    )
}

fn advance_presentation(
    state: &mut ProvisionalWindow,
    shared: &SharedHost,
    window: &mut Window,
    cx: &mut App,
) {
    let progress = (|| -> Result<()> {
        if state.reveal.is_none()
            && state.session.snapshot().phase() == WindowProvisionalSessionPhase::Gated
            && window.presentation_facts().initial_presentation
                == WindowInitialPresentationStatus::Completed
        {
            let request = physical_request(state.preview.placement, state.route, window, cx)?;
            state.reveal = Some(
                window.arm_provisional_presentation_with_initial_physical_placement(
                    &state.session,
                    request,
                    state.peers.iter().copied(),
                    cx,
                )?,
            );
        }
        if state.binding.is_some()
            && state.semantics.is_none()
            && state.placement.as_ref().is_some_and(|ticket| {
                let snapshot = ticket.snapshot();
                snapshot.outcome() == WindowProvisionalPlacementOutcome::Settled
                    && snapshot
                        .native_facts()
                        .is_some_and(|facts| facts.accepts_placement())
            })
        {
            state.semantics = Some(window.begin_provisional_destination_semantics(
                &state.session,
                state.session.snapshot().generation(),
                cx,
            )?);
        }
        Ok(())
    })();
    if let Err(error) = progress {
        log::warn!("native docking preview preparation failed: {error:#}");
        state.failure = Some(error.to_string());
        super::schedule(shared, cx);
    }
}

fn retire(shared: &SharedHost, state: Rc<RefCell<ProvisionalWindow>>, cx: &mut App) {
    let handle = state.borrow().window;
    let Some(handle) = handle else { return };
    let ticket = cx
        .native_window_retirement_ticket(handle.window_id())
        .or_else(|| {
            handle
                .update(cx, |_, window, cx| window.remove_window_with_retirement(cx))
                .ok()
        });
    let mut host = shared.borrow_mut();
    let provisional = &mut host.native.as_mut().unwrap().provisional;
    if let Some(ticket) = ticket {
        provisional.awaiting_retirement.remove(&handle.window_id());
        provisional.retirements.insert(handle.window_id(), ticket);
    } else {
        // A borrowed or already removed Window is not a native terminal. Retain
        // the original owner until GPUI publishes its real shutdown authority.
        provisional
            .awaiting_retirement
            .insert(handle.window_id(), state);
        let retry = cx.windows().contains(&handle);
        drop(host);
        if retry {
            super::schedule(shared, cx);
        }
    }
}

fn open_preview(
    shared: &SharedHost,
    drag: NativeDragTearOff,
    preview: Preview,
    pointer: PlatformNativePointerPhysicalFrame,
    capture: Option<NativeCapture>,
    peers: Vec<WindowId>,
    generation: u64,
    cx: &mut App,
) -> std::result::Result<Rc<RefCell<ProvisionalWindow>>, open_gpui::WindowOpenError> {
    let session =
        WindowProvisionalSession::new(generation).expect("nonzero provisional generation");
    let state = Rc::new(RefCell::new(ProvisionalWindow {
        window: None,
        drag,
        preview,
        session: session.clone(),
        reveal: None,
        placement: None,
        semantics: None,
        binding: None,
        admitted: false,
        failure: None,
        route: pointer,
        capture,
        release_frame: None,
        peers,
        placement_generation: 0,
        _initial: None,
    }));
    let root_state = state.clone();
    let dock = DockSurface {
        host: shared.clone(),
    };
    let options = WindowOptions {
        focus_on_appearing: false,
        provisional_session: Some(session),
        display_id: pointer.target_display().map(|display| display.display_id()),
        ..WindowOptions::default()
    };
    let window = cx.open_window_detailed(options, move |window, cx| {
        let weak = Rc::downgrade(&dock.host);
        let initial_state = Rc::downgrade(&root_state);
        let initial = window.observe_window_initial_presentation(move |window, cx| {
            window.refresh();
            if let Some(shared) = weak.upgrade() {
                if let Some(state) = initial_state.upgrade() {
                    // Hidden windows have no ordinary paint pump. Arm reveal
                    // from the original initial-presentation terminal itself.
                    advance_presentation(&mut state.borrow_mut(), &shared, window, cx);
                }
                super::schedule(&shared, cx);
            }
        });
        let weak = Rc::downgrade(&dock.host);
        let owner = window.window_handle().window_id();
        let closed = cx.on_window_closed(move |cx, closed| {
            if closed == owner
                && let Some(shared) = weak.upgrade()
            {
                super::schedule(&shared, cx);
            }
        });
        root_state.borrow_mut()._initial = Some(Subscription::join(initial, closed));
        window.on_window_should_close(cx, |_, _| false);
        cx.new(|_| NativeRoot {
            dock,
            surface: preview.target_surface,
            provisional: Some(root_state),
        })
    })?;
    state.borrow_mut().window = Some(window.into());
    if let Some(capture) = capture
        && let Some(callbacks) = shared
            .borrow()
            .native
            .as_ref()
            .and_then(|native| native.callbacks.get(&capture.source_window()))
    {
        callbacks.retain_drag_tear_off(capture, drag);
    }
    Ok(state)
}

/// Reconciles physical shells outside host borrows. Pending reveal retains the exact create FIFO.
pub(super) fn drive(shared: &SharedHost, bridge: &NativeBridge, cx: &mut App) -> bool {
    let unresolved: Vec<_> = shared
        .borrow()
        .native
        .as_ref()
        .unwrap()
        .provisional
        .awaiting_retirement
        .values()
        .cloned()
        .collect();
    for state in unresolved {
        retire(shared, state, cx);
    }
    let (retire_opening, desired, promotions) = {
        let mut host = shared.borrow_mut();
        let active = host.session.has_active_gesture();
        let pending =
            host.session.has_pending_native_create() || host.session.has_pending_presentation();
        let blocked = host.has_unaccepted_update() || host.session.has_pending_native_input();
        let native = host.native.as_mut().unwrap();
        let enabled = native.factory.is_none() && !native.transport.is_frozen();
        let provisional = &mut native.provisional;
        if provisional
            .preview
            .is_some_and(|preview| preview.capture != bridge.pointer_capture())
        {
            provisional.preview = None;
        }
        provisional.retirements.retain(|_, ticket| {
            let snapshot = ticket.snapshot();
            !(snapshot.quiesced() && snapshot.native_terminal())
        });
        let desired = (enabled && active && !blocked)
            .then_some(provisional.preview)
            .flatten();
        let keep = enabled && (active || pending || blocked);
        let retire = provisional.opening.as_ref().is_some_and(|state| {
            let state = state.borrow();
            !keep
                || desired.is_some_and(|preview| {
                    preview.drag != state.drag
                        || preview.visual.target_surface != state.preview.target_surface
                })
                || (!blocked && active && !pending && provisional.preview.is_none())
                || state.session.snapshot().phase() == WindowProvisionalSessionPhase::Terminal
                || state.failure.is_some()
                || state.reveal.as_ref().is_some_and(|ticket| {
                    !matches!(
                        ticket.snapshot().outcome(),
                        WindowProvisionalRevealOutcome::Pending
                            | WindowProvisionalRevealOutcome::Revealed
                    )
                })
        });
        let retire = if retire {
            let opening = provisional.opening.take();
            if let Some(state) = &opening {
                let state = state.borrow();
                if state.failure.is_some()
                    || state.reveal.as_ref().is_some_and(|ticket| {
                        !matches!(
                            ticket.snapshot().outcome(),
                            WindowProvisionalRevealOutcome::Pending
                                | WindowProvisionalRevealOutcome::Revealed
                        )
                    })
                {
                    provisional.failed = Some(state.preview.target_surface);
                }
            }
            opening
        } else {
            None
        };
        if !active {
            provisional.preview = None;
        }
        if !active && !pending {
            provisional.failed = None;
        }
        (
            retire,
            desired,
            provisional.promotions.values().cloned().collect::<Vec<_>>(),
        )
    };
    if let Some(state) = retire_opening {
        retire(shared, state, cx);
    }

    for state in promotions {
        let (handle, session, ticket, binding, first_live) = {
            let state = state.borrow();
            (
                state.window,
                state.session.clone(),
                state.semantics.clone(),
                state.binding,
                state.admitted,
            )
        };
        let Some(handle) = handle else { continue };
        if session.snapshot().phase() == WindowProvisionalSessionPhase::Terminal {
            shared
                .borrow_mut()
                .native
                .as_mut()
                .unwrap()
                .provisional
                .promotions
                .remove(&handle.window_id());
            continue;
        }
        if state.borrow().failure.is_some()
            || session.snapshot().phase()
                == WindowProvisionalSessionPhase::DestinationSemanticsRejected
        {
            let _ = handle.update(cx, |_, window, cx| window.remove_window(cx));
            continue;
        }
        let admitted = binding.is_some_and(|binding| {
            let host = shared.borrow();
            first_live
                && !host.native.as_ref().unwrap().transport.is_frozen()
                && host.session.is_current_native_binding(binding)
        });
        if session.snapshot().phase()
            == WindowProvisionalSessionPhase::DestinationSemanticsSubmitted
            && admitted
            && let Some(ticket) = ticket
        {
            match handle.update(cx, |_, window, cx| {
                window.admit_provisional_interaction(&session, &ticket, cx)
            }) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => shared.borrow_mut().fail(error.as_ref()),
                Err(error) => shared.borrow_mut().fail(error.as_ref()),
            }
        }
        if matches!(
            session.snapshot().phase(),
            WindowProvisionalSessionPhase::Promoted | WindowProvisionalSessionPhase::Terminal
        ) {
            shared
                .borrow_mut()
                .native
                .as_mut()
                .unwrap()
                .provisional
                .promotions
                .remove(&handle.window_id());
        }
    }

    if let Some(accepted) = desired {
        let preview = accepted.visual;
        let state = shared
            .borrow()
            .native
            .as_ref()
            .unwrap()
            .provisional
            .opening
            .clone();
        if let Some(state) = state {
            if let Some(pointer) = accepted.route {
                let mut state = state.borrow_mut();
                if state.preview != preview
                    && state.reveal.as_ref().is_some_and(|ticket| {
                        ticket.snapshot().outcome() == WindowProvisionalRevealOutcome::Revealed
                    })
                {
                    if let Some(handle) = state.window {
                        let result = handle.update(cx, |_, window, cx| -> Result<()> {
                            let request = placement_request(
                                &mut state,
                                preview.placement,
                                pointer,
                                WindowProvisionalPlacementPurpose::LiveRoute,
                                window,
                                cx,
                            )?;
                            let (_, ticket) = window
                                .request_provisional_live_placement(&state.session, request)?;
                            state.placement = Some(ticket);
                            state.preview = preview;
                            state.route = pointer;
                            wake_after_frame(shared, window);
                            Ok(())
                        });
                        if let Ok(Err(error)) = result {
                            shared.borrow_mut().fail(error.as_ref());
                        }
                    }
                }
            }
        } else if let Some(pointer) = accepted.route {
            let open = {
                let mut host = shared.borrow_mut();
                let native = host.native.as_mut().unwrap();
                if native.provisional.failed == Some(preview.target_surface)
                    || !cx
                        .window_capabilities()
                        .creation
                        .provisional_presentation
                        .is_supported()
                {
                    None
                } else {
                    native.provisional.generation = native
                        .provisional
                        .generation
                        .checked_add(1)
                        .expect("provisional generation exhausted");
                    Some((
                        native.provisional.generation,
                        native.callbacks.keys().copied().collect::<Vec<_>>(),
                    ))
                }
            };
            if let Some((generation, peers)) = open {
                let opened = open_preview(
                    shared,
                    accepted.drag,
                    preview,
                    pointer,
                    accepted.capture,
                    peers,
                    generation,
                    cx,
                );
                let mut host = shared.borrow_mut();
                let provisional = &mut host.native.as_mut().unwrap().provisional;
                match opened {
                    Ok(state) => provisional.opening = Some(state),
                    Err(error) => {
                        provisional.failed = Some(preview.target_surface);
                        if let Some(ticket) = error.retirement_ticket() {
                            provisional
                                .retirements
                                .insert(ticket.snapshot().window_id(), ticket.clone());
                        }
                        log::warn!("native docking preview open failed: {error:#}");
                    }
                }
            }
        }
    }
    let (dependencies, ready) = {
        let host = shared.borrow();
        let provisional = &host.native.as_ref().unwrap().provisional;
        let mut dependencies = provisional.retirements.keys().copied().collect::<Vec<_>>();
        dependencies.extend(provisional.awaiting_retirement.keys().copied());
        let ready = provisional.opening.as_ref().is_none_or(|state| {
            let state = state.borrow();
            if let Some(handle) = state.window {
                dependencies.push(handle.window_id());
            }
            state.reveal.as_ref().is_some_and(|ticket| {
                ticket.snapshot().outcome() != WindowProvisionalRevealOutcome::Pending
            })
        });
        (dependencies, ready)
    };
    let new_dependencies: Vec<_> = {
        let host = shared.borrow();
        let registered = &host.native.as_ref().unwrap().provisional.dependencies;
        dependencies
            .into_iter()
            .filter(|owner| !registered.contains(owner))
            .collect()
    };
    if !new_dependencies.is_empty() {
        match bridge.retain_root_retirement_dependencies(new_dependencies.iter().copied(), cx) {
            Ok(()) => shared
                .borrow_mut()
                .native
                .as_mut()
                .unwrap()
                .provisional
                .dependencies
                .extend(new_dependencies),
            Err(error) => shared.borrow_mut().fail(&error),
        }
    }
    ready
}

/// Adopts only the original drag's reserved preview; no root registration is minted.
pub(super) fn adopt(
    dock: &DockSurface,
    request: NativeWindowRequest,
    callbacks: &NativeCallbacks,
    cx: &mut App,
) -> Option<std::result::Result<AnyWindowHandle, NativeOpenFailure>> {
    let state = {
        let mut host = dock.host.borrow_mut();
        let provisional = &mut host.native.as_mut()?.provisional;
        if !provisional.opening.as_ref().is_some_and(|state| {
            let state = state.borrow();
            request.drag_tear_off == Some(state.drag)
                && state.preview.target_surface == request.binding.surface()
        }) {
            return None;
        }
        provisional.preview = None;
        provisional.opening.take()?
    };
    let handle = state.borrow().window?;
    let result = handle
        .update(cx, |_, window, cx| -> Result<()> {
            let mut state = state.borrow_mut();
            let pointer = state
                .release_frame
                .context("native create has no original release packet")?;
            let placement = placement_request(
                &mut state,
                request.outer_bounds,
                pointer,
                WindowProvisionalPlacementPurpose::FinalRelease,
                window,
                cx,
            )?;
            callbacks.install(window, cx)?;
            let (dispatch, ticket) =
                window.request_provisional_placement(&state.session, placement)?;
            callbacks.retain_provisional_placement(dispatch, ticket.clone());
            state.placement = Some(ticket);
            state.binding = Some(request.binding);
            window.refresh();
            wake_after_frame(&dock.host, window);
            Ok(())
        })
        .map_err(|error| anyhow!(error))
        .and_then(|result| result);
    match result {
        Ok(()) => {
            let mut host = dock.host.borrow_mut();
            let native = host.native.as_mut().unwrap();
            native
                .callbacks
                .insert(handle.window_id(), callbacks.clone());
            native
                .provisional
                .promotions
                .insert(handle.window_id(), state);
            Some(Ok(handle))
        }
        Err(error) => {
            retire(&dock.host, state, cx);
            let retirement = dock
                .host
                .borrow()
                .native
                .as_ref()
                .unwrap()
                .provisional
                .retirements
                .get(&handle.window_id())
                .cloned();
            Some(Err(NativeOpenFailure {
                root_binding: None,
                stage: None,
                message: error.to_string(),
                retirement_ticket: retirement,
                source: None,
            }))
        }
    }
}

pub(crate) fn begin_submission(
    host: &HostState,
    window: &mut Window,
    cx: &mut App,
) -> Result<WindowFrameSubmissionTicket> {
    if let Some(state) = host.native.as_ref().and_then(|native| {
        native
            .provisional
            .promotions
            .get(&window.window_handle().window_id())
    }) {
        if let Some(ticket) = &state.borrow().semantics {
            if matches!(
                ticket.frame_submission().snapshot().outcome(),
                WindowFrameSubmissionOutcome::Pending | WindowFrameSubmissionOutcome::Accepted
            ) {
                window.resume_provisional_destination_semantics(&state.borrow().session, ticket)?;
                return Ok(ticket.frame_submission().clone());
            }
        }
    }
    window.begin_exact_frame_submission(cx)
}

pub(crate) fn accept_submission(
    host: &HostState,
    ticket: &WindowFrameSubmissionTicket,
    generation: u64,
    window: &mut Window,
    cx: &mut App,
) -> Result<()> {
    if let Some(state) = host.native.as_ref().and_then(|native| {
        native
            .provisional
            .promotions
            .get(&window.window_handle().window_id())
    }) {
        let state = state.borrow();
        if let Some(semantics) = state
            .semantics
            .as_ref()
            .filter(|semantics| semantics.frame_submission().same_authority(ticket))
        {
            window.accept_provisional_destination_semantics_frame(
                &state.session,
                semantics,
                generation,
                cx,
            )?;
            return Ok(());
        }
    }
    window.accept_exact_frame_submission_frame(ticket, generation, cx)?;
    Ok(())
}
