use super::*;
use crate::AppleDispatcher;
use objc2::rc::{Weak, autoreleasepool};
use open_gpui::{BackgroundExecutor, PlatformDispatcher};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
    thread::{self, ThreadId},
};

thread_local! {
    static ACTIVITIES: RefCell<Vec<ObservedActivity>> = const { RefCell::new(Vec::new()) };
}

pub(super) type ReleaseObservation = Arc<Mutex<Vec<EndedActivity>>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EndedActivity {
    process_info: usize,
    activity: usize,
    thread: ThreadId,
}

#[derive(Debug)]
struct ObservedActivity {
    process_info: Weak<NSProcessInfo>,
    activity: Weak<ProtocolObject<dyn NSObjectProtocol>>,
    process_info_address: usize,
    activity_address: usize,
    reason: String,
    options: NSActivityOptions,
    releases: ReleaseObservation,
}

pub(super) fn observe_begin(
    process_info: &Retained<NSProcessInfo>,
    activity: &Retained<ProtocolObject<dyn NSObjectProtocol>>,
    reason: &str,
    options: NSActivityOptions,
) -> ReleaseObservation {
    let releases = Arc::new(Mutex::new(Vec::new()));
    ACTIVITIES.with_borrow_mut(|activities| {
        activities.push(ObservedActivity {
            process_info: Weak::from_retained(process_info),
            activity: Weak::from_retained(activity),
            process_info_address: Retained::as_ptr(process_info) as usize,
            activity_address: Retained::as_ptr(activity) as usize,
            reason: reason.to_owned(),
            options,
            releases: releases.clone(),
        });
    });
    releases
}

pub(super) fn observe_end(
    releases: &ReleaseObservation,
    process_info: &Retained<NSProcessInfo>,
    activity: &Retained<ProtocolObject<dyn NSObjectProtocol>>,
) {
    releases.lock().unwrap().push(EndedActivity {
        process_info: Retained::as_ptr(process_info) as usize,
        activity: Retained::as_ptr(activity) as usize,
        thread: thread::current().id(),
    });
}

fn take_activities() -> Vec<ObservedActivity> {
    ACTIVITIES.with_borrow_mut(std::mem::take)
}

impl ObservedActivity {
    fn assert_active(&self) {
        let releases = self.releases.lock().unwrap().clone();
        assert!(releases.is_empty());
        let process_info = self
            .process_info
            .load()
            .expect("process info remains alive");
        let activity = self.activity.load().expect("native activity remains alive");
        assert_eq!(
            Retained::as_ptr(&process_info) as usize,
            self.process_info_address
        );
        assert_eq!(Retained::as_ptr(&activity) as usize, self.activity_address);
    }

    fn assert_ended_once_on(&self, thread: ThreadId) {
        let releases = self.releases.lock().unwrap().clone();
        assert_eq!(
            releases,
            vec![EndedActivity {
                process_info: self.process_info_address,
                activity: self.activity_address,
                thread,
            }]
        );
    }
}

#[test]
fn dispatcher_keeps_independent_app_nap_activities_alive_outside_the_autorelease_pool() {
    let (first, second) = autoreleasepool(|_| {
        let dispatcher = AppleDispatcher::new();
        (
            dispatcher.prevent_app_nap("first native activity"),
            dispatcher.prevent_app_nap("second native activity"),
        )
    });
    let [first_activity, second_activity]: [ObservedActivity; 2] =
        take_activities().try_into().unwrap();
    assert_eq!(first_activity.reason, "first native activity");
    assert_eq!(second_activity.reason, "second native activity");
    assert_ne!(
        first_activity.activity_address,
        second_activity.activity_address
    );
    for activity in [&first_activity, &second_activity] {
        assert_eq!(
            activity.options,
            NSActivityOptions::UserInitiatedAllowingIdleSystemSleep
        );
        assert!(
            !activity
                .options
                .contains(NSActivityOptions::IdleSystemSleepDisabled)
        );
        activity.assert_active();
    }

    drop(first);
    first_activity.assert_ended_once_on(thread::current().id());
    second_activity.assert_active();
    drop(second);
    first_activity.assert_ended_once_on(thread::current().id());
    second_activity.assert_ended_once_on(thread::current().id());
}

#[test]
fn background_executor_guard_outlives_its_executor_and_ends_on_another_thread() {
    let guard = autoreleasepool(|_| {
        let executor = BackgroundExecutor::new(Arc::new(AppleDispatcher::new()));
        executor.prevent_app_nap("background activity β")
    });
    let [activity]: [ObservedActivity; 1] = take_activities().try_into().unwrap();
    assert_eq!(activity.reason, "background activity β");
    assert_eq!(
        activity.options,
        NSActivityOptions::UserInitiatedAllowingIdleSystemSleep
    );
    activity.assert_active();

    let release_thread = thread::spawn(move || {
        autoreleasepool(|_| {
            drop(guard);
            thread::current().id()
        })
    })
    .join()
    .unwrap();
    assert_ne!(release_thread, thread::current().id());
    activity.assert_ended_once_on(release_thread);
}

#[test]
#[cfg(panic = "unwind")]
fn unwinding_an_inner_activity_preserves_the_outer_native_activity() {
    let outer = autoreleasepool(|_| {
        let dispatcher = AppleDispatcher::new();
        let outer = dispatcher.prevent_app_nap("outer activity");
        let result = std::panic::catch_unwind(|| {
            let _inner = dispatcher.prevent_app_nap("inner activity");
            panic!("activity guard unwind regression");
        });
        assert!(result.is_err());
        outer
    });
    let [outer_activity, inner_activity]: [ObservedActivity; 2] =
        take_activities().try_into().unwrap();
    inner_activity.assert_ended_once_on(thread::current().id());
    outer_activity.assert_active();
    drop(outer);
    inner_activity.assert_ended_once_on(thread::current().id());
    outer_activity.assert_ended_once_on(thread::current().id());
}
