//! Application observations copied before the host consumes committed reports.

use std::{
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

use anyhow::{Result, anyhow};
use dockspace::runtime::{DockspaceNotification, DockspacePresentationTransition, HostFrameReport};
use open_gpui::App;

use super::{DockSurface, HostState, SharedHost};

/// Read-only application changes and presentation terminals from one commit.
///
/// This contains no native effects, painted outputs or presentation authority.
/// Rejected, stale and unchanged actions do not manufacture product events.
/// A presentation failure can have an empty notification and still carry its
/// original transition terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockCommitNotification {
    notification: DockspaceNotification,
}

impl DockCommitNotification {
    /// Returns the core's complete notification, preserving its event order.
    pub fn notification(&self) -> &DockspaceNotification {
        &self.notification
    }

    /// Returns the presentation terminals settled by this commit in core order.
    ///
    /// Match each terminal's identity to the transition returned by
    /// [`DockSurface::update_action`], including requests from earlier commits.
    pub fn presentation_transitions(&self) -> &[DockspacePresentationTransition] {
        self.notification.presentation_transitions()
    }
}

pub(super) type CommitHandler = dyn Fn(&DockSurface, DockCommitNotification, &mut App);

impl DockSurface {
    /// Installs the application's observer for subsequent committed changes.
    ///
    /// Register before issuing actions or opening windows to observe their
    /// changes. Commits without a handler retain no notification history.
    /// Empty notifications without presentation terminals are omitted.
    ///
    /// Delivery uses [`App::defer`] outside the host borrow and preserves commit
    /// and core event order. The current model can already include later
    /// commits when a callback runs. The supplied surface avoids capturing a
    /// strong clone in its own handler.
    ///
    /// The handler may inspect or update the surface and replace or remove
    /// itself. Replacement applies to the next queued notification. Delivery
    /// processes a finite batch at a time and preserves reentrant commit order.
    /// Notification delivery neither consumes native work nor changes an
    /// action's success, rejection or eventual presentation result.
    /// A handler panic propagates after scheduling remaining notifications for
    /// a later application update; the delivered notification is not replayed.
    pub fn set_commit_handler(
        &self,
        handler: impl Fn(&DockSurface, DockCommitNotification, &mut App) + 'static,
        cx: &mut App,
    ) -> Result<()> {
        let mut host = self
            .host
            .try_borrow_mut()
            .map_err(|_| anyhow!("docking host is busy"))?;
        host.commit_handler = Some(Rc::new(handler));
        drop(host);
        schedule(&self.host, cx);
        Ok(())
    }

    /// Removes the observer and discards its undelivered notification copies.
    ///
    /// Installing another handler later does not replay these notifications or
    /// any commits made while no handler was installed.
    pub fn clear_commit_handler(&self) -> Result<()> {
        let mut host = self
            .host
            .try_borrow_mut()
            .map_err(|_| anyhow!("docking host is busy"))?;
        host.commit_handler = None;
        host.commit_notifications.clear();
        Ok(())
    }
}

pub(super) fn observe(host: &mut HostState, report: &HostFrameReport) {
    if host.commit_handler.is_none() || report.notification().is_empty() {
        return;
    }
    host.commit_notifications.push_back(DockCommitNotification {
        notification: report.notification().clone(),
    });
}

pub(super) fn schedule(shared: &SharedHost, cx: &mut App) {
    {
        let Ok(mut host) = shared.try_borrow_mut() else {
            return;
        };
        if host.commit_handler.is_none()
            || host.commit_notifications.is_empty()
            || host.commit_dispatching
            || host.commit_pump_scheduled
        {
            return;
        }
        host.commit_pump_scheduled = true;
    }
    let weak = Rc::downgrade(shared);
    cx.defer(move |cx| {
        if let Some(shared) = weak.upgrade() {
            dispatch(&shared, cx);
        }
    });
}

fn dispatch(shared: &SharedHost, cx: &mut App) {
    let count = {
        let mut host = shared.borrow_mut();
        host.commit_pump_scheduled = false;
        if host.commit_dispatching || host.commit_handler.is_none() {
            return;
        }
        host.commit_dispatching = true;
        host.commit_notifications.len()
    };
    let guard = DispatchGuard(shared.clone());
    let dock = DockSurface {
        host: shared.clone(),
    };
    let mut panic_payload = None;
    for _ in 0..count {
        let next = {
            let mut host = shared.borrow_mut();
            host.commit_handler.clone().and_then(|handler| {
                host.commit_notifications
                    .pop_front()
                    .map(|notification| (handler, notification))
            })
        };
        let Some((handler, notification)) = next else {
            break;
        };
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| handler(&dock, notification, cx))) {
            panic_payload = Some(payload);
            break;
        }
    }
    drop(guard);
    if !shared.borrow().commit_notifications.is_empty() {
        schedule(shared, cx);
    }
    if let Some(payload) = panic_payload {
        resume_unwind(payload);
    }
}

struct DispatchGuard(SharedHost);

impl Drop for DispatchGuard {
    fn drop(&mut self) {
        self.0.borrow_mut().commit_dispatching = false;
    }
}

#[cfg(test)]
#[path = "notification_tests.rs"]
mod tests;
