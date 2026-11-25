//! Uses the production async path and Objective-C reply selector without exiting the test runner.

use super::*;
use objc2::{AnyThread, DefinedClass, define_class};
use objc2_foundation::{NSObject, NSObjectProtocol};
use open_gpui::TestAppContext;

define_class!(
    // SAFETY: NSObject has no additional subclassing requirements. The reply log is synchronized,
    // and this responder deliberately implements only AppKit's termination reply selector.
    #[unsafe(super(NSObject))]
    #[ivars = Arc<Mutex<Vec<bool>>>]
    struct TerminationReplyProbe;

    unsafe impl NSObjectProtocol for TerminationReplyProbe {}

    impl TerminationReplyProbe {
        #[unsafe(method(replyToApplicationShouldTerminate:))]
        fn reply(&self, allow: Bool) {
            self.ivars().lock().push(allow.as_bool());
        }
    }
);

impl TerminationReplyProbe {
    fn new(replies: Arc<Mutex<Vec<bool>>>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(replies);
        unsafe { msg_send![super(this), init] }
    }
}

fn termination(cx: &TestAppContext) -> Rc<MacApplicationTermination> {
    Rc::new(MacApplicationTermination::new(
        cx.foreground_executor().clone(),
        cx.executor(),
    ))
}

#[open_gpui::test]
fn native_termination_retries_false_and_coalesces_repeated_requests(cx: &mut TestAppContext) {
    let termination = termination(cx);
    let replies = Arc::new(Mutex::new(Vec::new()));
    let application = TerminationReplyProbe::new(replies.clone());
    let attempts = Rc::new(Cell::new(0));
    termination.set_callback(Box::new({
        let attempts = attempts.clone();
        let cx = cx.clone();
        move || {
            cx.update(|_| {
                attempts.set(attempts.get() + 1);
                attempts.get() > 1
            })
        }
    }));

    cx.update(|_| {
        for _ in 0..3 {
            assert_eq!(
                termination.request(&application),
                NSApplicationTerminateReply::TerminateLater
            );
        }
    });
    assert_eq!(
        attempts.get(),
        0,
        "native ingress must not enter App inline"
    );
    cx.run_until_parked();
    assert_eq!(attempts.get(), 1);
    assert!(replies.lock().is_empty(), "false is not permission to exit");
    assert_eq!(
        termination.request(&application),
        NSApplicationTerminateReply::TerminateLater
    );
    cx.executor().advance_clock(RETRY_DELAY);
    cx.run_until_parked();
    assert_eq!(attempts.get(), 2);
    assert_eq!(replies.lock().as_slice(), &[true]);
    cx.executor().advance_clock(RETRY_DELAY);
    cx.run_until_parked();
    assert_eq!(replies.lock().as_slice(), &[true]);
}

#[open_gpui::test]
fn native_termination_cancel_withdraws_old_retry_before_a_new_request(cx: &mut TestAppContext) {
    for cancel_during_callback in [false, true] {
        let termination = termination(cx);
        let old_replies = Arc::new(Mutex::new(Vec::new()));
        let old_application = TerminationReplyProbe::new(old_replies.clone());
        termination.set_callback(Box::new({
            let termination = Rc::downgrade(&termination);
            move || {
                if cancel_during_callback {
                    termination.upgrade().unwrap().cancel();
                }
                cancel_during_callback
            }
        }));
        assert_eq!(
            termination.request(&old_application),
            NSApplicationTerminateReply::TerminateLater
        );
        cx.run_until_parked();
        if !cancel_during_callback {
            termination.cancel();
        }
        assert_eq!(old_replies.lock().as_slice(), &[false]);

        let new_replies = Arc::new(Mutex::new(Vec::new()));
        let new_application = TerminationReplyProbe::new(new_replies.clone());
        termination.set_callback(Box::new(|| true));
        assert_eq!(
            termination.request(&new_application),
            NSApplicationTerminateReply::TerminateLater
        );
        cx.run_until_parked();
        assert_eq!(new_replies.lock().as_slice(), &[true]);
        cx.executor().advance_clock(RETRY_DELAY);
        cx.run_until_parked();
        assert_eq!(old_replies.lock().as_slice(), &[false]);
        assert_eq!(new_replies.lock().as_slice(), &[true]);
    }
}

#[open_gpui::test]
fn native_termination_callback_replacement_rejects_the_old_success(cx: &mut TestAppContext) {
    let termination = termination(cx);
    let replies = Arc::new(Mutex::new(Vec::new()));
    let application = TerminationReplyProbe::new(replies.clone());
    termination.set_callback(Box::new({
        let termination = Rc::downgrade(&termination);
        move || {
            termination
                .upgrade()
                .unwrap()
                .set_callback(Box::new(|| true));
            true
        }
    }));
    assert_eq!(
        termination.request(&application),
        NSApplicationTerminateReply::TerminateLater
    );
    cx.run_until_parked();
    assert!(replies.lock().is_empty());
    cx.executor().advance_clock(RETRY_DELAY);
    cx.run_until_parked();
    assert_eq!(replies.lock().as_slice(), &[true]);
}

#[open_gpui::test]
fn native_termination_callback_panic_does_not_confirm_shutdown(cx: &mut TestAppContext) {
    let termination = termination(cx);
    let replies = Arc::new(Mutex::new(Vec::new()));
    let application = TerminationReplyProbe::new(replies.clone());
    termination.set_callback(Box::new({
        let panicked = Cell::new(false);
        move || {
            if !panicked.replace(true) {
                panic!("injected native termination callback panic");
            }
            true
        }
    }));
    assert_eq!(
        termination.request(&application),
        NSApplicationTerminateReply::TerminateLater
    );
    cx.run_until_parked();
    assert!(replies.lock().is_empty());
    cx.executor().advance_clock(RETRY_DELAY);
    cx.run_until_parked();
    assert_eq!(replies.lock().as_slice(), &[true]);
}
