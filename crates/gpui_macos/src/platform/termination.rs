//! Defers AppKit termination until the original quit callback confirms native shutdown.

use super::*;
use objc2::Message as _;
use objc2_foundation::{NSObject, NSObjectNSDelayedPerforming, NSRunLoopCommonModes};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    time::Duration,
};

const RETRY_DELAY: Duration = Duration::from_millis(16);

#[cfg(test)]
mod tests;

pub(super) struct MacApplicationTermination {
    callback: RefCell<Option<Box<dyn FnMut() -> bool>>>,
    pending: RefCell<Option<Rc<TerminationRequest>>>,
    scheduled_application: RefCell<Option<Retained<NSApplication>>>,
    foreground: ForegroundExecutor,
    background: BackgroundExecutor,
}

struct TerminationRequest {
    application: Retained<AnyObject>,
}

impl TerminationRequest {
    fn reply(&self, allow: bool) {
        // The reply may synchronously enter applicationWillTerminate. Its request is already
        // removed, and neither the platform mutex nor a callback borrow remains held here.
        unsafe {
            let _: () = msg_send![&*self.application,
                replyToApplicationShouldTerminate: allow.to_objc()
            ];
        }
    }
}

impl MacApplicationTermination {
    pub(super) fn new(foreground: ForegroundExecutor, background: BackgroundExecutor) -> Self {
        Self {
            callback: RefCell::new(None),
            pending: RefCell::new(None),
            scheduled_application: RefCell::new(None),
            foreground,
            background,
        }
    }

    pub(super) fn set_callback(&self, callback: Box<dyn FnMut() -> bool>) {
        let previous = self.callback.replace(Some(callback));
        isolate_panic(|| drop(previous));
    }

    pub(super) fn request_quit(&self, application: &NSApplication) {
        if self.pending.borrow().is_some() || self.scheduled_application.borrow().is_some() {
            return;
        }
        self.scheduled_application
            .replace(Some(application.retain()));
        // terminate: may run a nested event loop while waiting for TerminateLater. Enter it from
        // a run-loop timer, not the main dispatch queue: that queue cannot reenter its own block,
        // and GPUI's shutdown work and this request's checks both need it to remain dispatchable.
        // A zero-delay perform also releases the current App borrow before termination begins.
        unsafe {
            application.performSelector_withObject_afterDelay_inModes(
                sel!(terminate:),
                None,
                0.0,
                &NSArray::from_slice(&[NSRunLoopCommonModes]),
            );
        }
    }

    fn request(self: &Rc<Self>, application: &AnyObject) -> NSApplicationTerminateReply {
        self.scheduled_application.take();
        if self.pending.borrow().is_some() {
            return NSApplicationTerminateReply::TerminateLater;
        }
        if self.callback.borrow().is_none() {
            return NSApplicationTerminateReply::TerminateNow;
        }
        let request = Rc::new(TerminationRequest {
            application: application.retain(),
        });
        self.pending.replace(Some(request.clone()));
        self.schedule(request, Duration::ZERO);
        NSApplicationTerminateReply::TerminateLater
    }

    fn schedule(self: &Rc<Self>, request: Rc<TerminationRequest>, delay: Duration) {
        let termination = Rc::downgrade(self);
        let background = self.background.clone();
        self.foreground
            .spawn(async move {
                if !delay.is_zero() {
                    background.timer(delay).await;
                }
                if let Some(termination) = termination.upgrade() {
                    termination.check(request);
                }
            })
            .detach();
    }

    fn is_current(&self, request: &Rc<TerminationRequest>) -> bool {
        self.pending
            .borrow()
            .as_ref()
            .is_some_and(|pending| Rc::ptr_eq(pending, request))
    }

    fn check(self: &Rc<Self>, request: Rc<TerminationRequest>) {
        if !self.is_current(&request) {
            return;
        }
        let callback = self.callback.borrow_mut().take();
        let Some(mut callback) = callback else {
            self.pending.take();
            request.reply(false);
            return;
        };
        let complete = isolate_panic(&mut callback).unwrap_or(false);
        if !self.is_current(&request) {
            isolate_panic(|| drop(callback));
            return;
        }
        // A replacement registered while the old callback ran owns the next decision. Restoring
        // the checked-out callback must not overwrite it or trust the obsolete callback's result.
        let replaced = self.callback.borrow().is_some();
        if replaced {
            isolate_panic(|| drop(callback));
        } else {
            self.callback.replace(Some(callback));
        }
        if complete && !replaced {
            self.pending.take();
            request.reply(true);
        } else {
            self.schedule(request, RETRY_DELAY);
        }
    }

    pub(super) fn cancel(&self) {
        self.cancel_scheduled_quit();
        let pending = self.pending.take();
        let callback = self.callback.take();
        isolate_panic(|| drop(callback));
        if let Some(pending) = pending {
            pending.reply(false);
        }
    }

    pub(super) fn did_terminate(&self) {
        self.cancel_scheduled_quit();
        self.pending.take();
        let callback = self.callback.take();
        isolate_panic(|| drop(callback));
    }

    fn cancel_scheduled_quit(&self) {
        if let Some(application) = self.scheduled_application.take() {
            // A host can stop app.run() without terminating the process. Retract its delayed
            // selector so a later run loop cannot execute an obsolete platform exit request.
            unsafe {
                NSObject::cancelPreviousPerformRequestsWithTarget_selector_object(
                    &application,
                    sel!(terminate:),
                    None,
                );
            }
        }
    }
}

fn isolate_panic<T>(callback: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(result) => Some(result),
        Err(payload) => {
            // A panic is not proof that shutdown completed. Avoid a second unwind from an
            // arbitrary panic-payload destructor at the native boundary and retry the callback.
            std::mem::forget(payload);
            log::error!("isolated a panic while waiting for macOS application shutdown");
            None
        }
    }
}

pub(super) extern "C-unwind" fn should_terminate(
    this: &AnyObject,
    _: Sel,
    application: *mut AnyObject,
) -> NSApplicationTerminateReply {
    let platform = unsafe { get_mac_platform(this) };
    let termination = platform.0.lock().termination.clone();
    termination.request(unsafe { &*application })
}
