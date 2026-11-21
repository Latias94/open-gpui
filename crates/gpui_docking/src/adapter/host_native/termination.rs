//! Failed generations retain their semantic owners while the backend retires.

use super::*;
use crate::adapter::{DockHostFailure, DockNativeTermination};

pub(super) type TerminationCallback = Box<dyn FnOnce(DockNativeTermination, &mut App)>;

pub(super) struct FailedShutdown {
    failure: DockHostFailure,
    terminated: bool,
}

pub(super) fn disposition(host: &HostState, native: &ManagedHost) -> Option<DockNativeTermination> {
    if let Some(failed) = &native.failed_shutdown {
        return failed
            .terminated
            .then(|| DockNativeTermination::Failed(failed.failure.clone()));
    }
    is_quiescent(host, native).then_some(DockNativeTermination::Drained)
}

impl DockSurface {
    /// Irreversibly terminates a failed native generation without resuming its FIFO.
    ///
    /// Choose this explicitly when repairing the original protocol is no longer
    /// appropriate. A classified protocol/internal failure must already exist.
    /// A failure category alone never starts this path automatically. Pending
    /// records, affine results and outputs remain with the old session until the
    /// complete generation is dropped after [`Self::native_termination`].
    ///
    /// Dependent windows retire before roots and the anchor. Actual capture,
    /// renderer, native and asynchronous terminals remain mandatory. This does
    /// not claim semantic drain, request App exit or close sibling workspaces.
    /// App's last-window exit policy still applies. Applications reconstructing
    /// in the same process must retain a recovery window or select a suitable
    /// last-window policy before starting termination.
    pub fn begin_failed_native_shutdown(&self, cx: &mut App) -> Result<()> {
        {
            let mut host = self
                .host
                .try_borrow_mut()
                .map_err(|_| anyhow!("docking host is busy"))?;
            let failure = host
                .failure
                .as_ref()
                .filter(|failure| failure.is_primary_protocol_failure())
                .cloned()
                .ok_or_else(|| {
                    anyhow!("native failure termination requires an original protocol failure")
                })?;
            let native = host
                .native
                .as_mut()
                .ok_or_else(|| anyhow!("managed native hosting is not enabled"))?;
            anyhow::ensure!(native.bridge.is_some(), "native dispatch is busy");
            if native.failed_shutdown.is_none() {
                native.failed_shutdown = Some(FailedShutdown {
                    failure,
                    terminated: false,
                });
            }
            native.transport.freeze_admission();
            native.wake = None;
            native.blocked_continuation = None;
            native._displays.take();
            for mount in host.mounts.values_mut() {
                mount.wake = None;
            }
        }
        pump_failed(&self.host, cx);
        Ok(())
    }

    /// Observes successful drain or complete physical termination of a failed generation.
    ///
    /// `Failed` leaves the original session and its unconsumable records intact.
    /// Reconstruct from committed logical state and application pane data in a
    /// fresh managed session; never transfer old bindings, results or outputs.
    pub fn native_termination(&self) -> Option<DockNativeTermination> {
        let host = self.host.borrow();
        let native = host.native.as_ref()?;
        disposition(&host, native)
    }

    /// Runs once outside host borrows after the original generation terminates.
    ///
    /// The disposition distinguishes a failed generation from semantic drain.
    /// Capture weak application owners to avoid retaining the workspace through
    /// its own callback. Already terminated workspaces defer the callback via App.
    pub fn when_native_terminated(
        &self,
        callback: impl FnOnce(DockNativeTermination, &mut App) + 'static,
        cx: &mut App,
    ) -> Result<()> {
        let mut host = self
            .host
            .try_borrow_mut()
            .map_err(|_| anyhow!("docking host is busy"))?;
        let native = host
            .native
            .as_ref()
            .ok_or_else(|| anyhow!("managed native hosting is not enabled"))?;
        let termination = disposition(&host, native);
        if let Some(termination) = termination {
            drop(host);
            cx.defer(move |cx| callback(termination, cx));
            return Ok(());
        }
        let native = host.native.as_mut().unwrap();
        native.termination_callbacks.push(Box::new(callback));
        drop(host);
        schedule(&self.host, cx);
        Ok(())
    }
}

pub(super) fn pump_failed(shared: &SharedHost, cx: &mut App) {
    let Ok(mut host) = shared.try_borrow_mut() else {
        return;
    };
    let Some(native) = host.native.as_mut() else {
        return;
    };
    if native
        .failed_shutdown
        .as_ref()
        .is_none_or(|failed| failed.terminated)
    {
        return;
    }
    let Some(mut bridge) = native.bridge.take() else {
        return;
    };
    drop(host);

    // This retires visual-only shells and observes promoted owners. It cannot
    // create or admit another shell after the original transport is frozen.
    provisional::drive(shared, &bridge, cx);
    let mut retry_borrow =
        bridge.failed_children_terminated() && retire_unbound_roots(shared, false, cx);
    let external_roots_ready = {
        let host = shared.borrow();
        let native = host.native.as_ref().unwrap();
        native.provisional.is_quiescent()
            && native
                .registrations
                .iter()
                .filter(|(_, root)| !root.anchor)
                .all(|(owner, _)| native.retirements.get(owner).is_some_and(ticket_terminated))
            && native.retirements.values().all(ticket_terminated)
    };
    match bridge.drive_failed_shutdown(external_roots_ready, cx) {
        Ok(retry) => retry_borrow |= retry,
        Err(error) => shared.borrow_mut().fail(&error),
    }
    if bridge.failed_shutdown_complete() {
        retry_borrow |= retire_unbound_roots(shared, true, cx);
    }

    let mut host = shared.borrow_mut();
    let native = host.native.as_mut().unwrap();
    let terminated = bridge.failed_shutdown_complete()
        && native.provisional.is_quiescent()
        && native.retirements.values().all(ticket_terminated)
        && native
            .registrations
            .keys()
            .all(|owner| native.retirements.get(owner).is_some_and(ticket_terminated))
        && native
            .callbacks
            .values()
            .all(NativeCallbacks::physical_terminated);
    native.bridge = Some(bridge);
    let callbacks = if terminated {
        native._quit.take();
        native._terminal.take();
        native.failed_shutdown.as_mut().unwrap().terminated = true;
        std::mem::take(&mut native.termination_callbacks)
    } else {
        Vec::new()
    };
    let failure = native.failed_shutdown.as_ref().unwrap().failure.clone();
    if terminated {
        for mount in host.mounts.values_mut() {
            mount._lifecycle_observers.take();
        }
    }
    drop(host);
    if retry_borrow {
        schedule(shared, cx);
    }
    for callback in callbacks {
        callback(DockNativeTermination::Failed(failure.clone()), cx);
    }
}

fn ticket_terminated(ticket: &open_gpui::WindowPresentationShutdownTicket) -> bool {
    let snapshot = ticket.snapshot();
    snapshot.quiesced() && snapshot.native_terminal()
}

fn retire_unbound_roots(shared: &SharedHost, anchor: bool, cx: &mut App) -> bool {
    let roots: Vec<_> = {
        let host = shared.borrow();
        let native = host.native.as_ref().unwrap();
        native
            .registrations
            .iter()
            .filter(|(owner, root)| {
                root.anchor == anchor && !native.retirements.contains_key(owner)
            })
            .map(|(owner, root)| (*owner, root.window))
            .collect()
    };
    let mut retry = false;
    for (owner, window) in roots {
        let ticket = cx.native_window_retirement_ticket(owner).or_else(|| {
            window
                .update(cx, |_, window, cx| window.remove_window_with_retirement(cx))
                .ok()
        });
        if let Some(ticket) = ticket {
            shared
                .borrow_mut()
                .native
                .as_mut()
                .unwrap()
                .retirements
                .insert(owner, ticket);
        } else {
            retry |= cx.windows().contains(&window);
        }
    }
    retry
}
