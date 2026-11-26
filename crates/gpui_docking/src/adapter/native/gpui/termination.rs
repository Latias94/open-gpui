//! Physical termination never consumes or repairs the failed semantic prefix.

use super::*;

impl NativeCallbacks {
    pub(in crate::adapter) fn physical_terminated(&self) -> bool {
        self.0.sealed.get()
            && self
                .0
                .open_retirement
                .borrow()
                .as_ref()
                .is_some_and(|ticket| {
                    let snapshot = ticket.snapshot();
                    snapshot.quiesced() && snapshot.native_terminal()
                })
    }

    fn retire_failed(&self, cx: &mut App) -> bool {
        if self.physical_terminated() {
            return false;
        }
        let mut retry_borrow = false;
        if self.0.open_retirement.borrow().is_none() {
            let ticket = self
                .window_id()
                .and_then(|owner| cx.native_window_retirement_ticket(owner))
                .or_else(|| {
                    self.cancel_capture(cx);
                    if self.0.capture.get().is_some()
                        || self.0.capture_start.get().is_some()
                        || !self.0.capture_releases.borrow().is_empty()
                        || !self.0.capture_cancellations.borrow().is_empty()
                        || self.0.capture_failed.get()
                    {
                        return None;
                    }
                    let handle = self.handle()?;
                    match handle
                        .update(cx, |_, window, cx| window.remove_window_with_retirement(cx))
                    {
                        Ok(ticket) => Some(ticket),
                        Err(_) => {
                            retry_borrow = cx.windows().contains(&handle);
                            None
                        }
                    }
                });
            if let Some(ticket) = ticket {
                // Retain the original App/renderer authority, including retries
                // after logical removal or a failed construction transaction.
                self.retain_open_retirement(ticket, cx);
            }
        }
        self.observe_open_retirement(cx);
        let _ = self.seal();
        retry_borrow
    }
}

impl NativeBridge {
    pub(in crate::adapter) fn drive_failed_shutdown(
        &mut self,
        external_roots_ready: bool,
        cx: &mut App,
    ) -> Result<bool, NativeBridgeError> {
        self.draining = true;
        self.service_failed_capture_terminals();
        // A retired binding can no longer deliver its original ticket. Settle
        // its dispatch even when the failed semantic prefix will not be retried.
        self.finish_pending(cx);
        let callbacks: Vec<_> = self
            .windows
            .iter()
            .map(|(_, _, callbacks)| callbacks.clone())
            .chain(self.opening.iter().cloned())
            .collect();
        let mut retry = false;
        if !self.failed_children_terminated() {
            for callbacks in callbacks
                .iter()
                .filter(|callbacks| callbacks.is_child_window())
            {
                retry |= callbacks.retire_failed(cx);
            }
            if !self.failed_children_terminated() {
                return Ok(retry);
            }
            // Sealing the final child can complete synchronously. Wake the host
            // so it can now retire any external roots before closing the anchor.
            retry = true;
        }
        for callbacks in callbacks
            .iter()
            .filter(|callbacks| Some(callbacks.binding()) != self.anchor)
        {
            retry |= callbacks.retire_failed(cx);
        }
        if external_roots_ready
            && callbacks
                .iter()
                .filter(|callbacks| Some(callbacks.binding()) != self.anchor)
                .all(NativeCallbacks::physical_terminated)
        {
            for callbacks in callbacks
                .iter()
                .filter(|callbacks| Some(callbacks.binding()) == self.anchor)
            {
                retry |= callbacks.retire_failed(cx);
            }
        }
        Ok(retry)
    }

    pub(in crate::adapter) fn failed_children_terminated(&self) -> bool {
        self.windows.iter().all(|(_, _, callbacks)| {
            !callbacks.is_child_window() || callbacks.physical_terminated()
        }) && self
            .opening
            .as_ref()
            .is_none_or(|callbacks| !callbacks.is_child_window() || callbacks.physical_terminated())
    }

    pub(in crate::adapter) fn failed_shutdown_complete(&self) -> bool {
        self.effects.pending().len() == 0
            && self
                .windows
                .iter()
                .all(|(_, _, callbacks)| callbacks.physical_terminated())
            && self
                .opening
                .as_ref()
                .is_none_or(NativeCallbacks::physical_terminated)
            && self.transport.callbacks_idle()
    }
}
