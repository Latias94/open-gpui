use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::{NSActivityOptions, NSObjectProtocol, NSProcessInfo, NSString};
use open_gpui::ActivityGuard;

pub(crate) struct AppleActivity {
    process_info: Retained<NSProcessInfo>,
    activity: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    #[cfg(test)]
    release_observation: tests::ReleaseObservation,
}

// The activity token returned by NSProcessInfo is thread-safe
unsafe impl Send for AppleActivity {}

impl AppleActivity {
    pub(crate) fn begin(reason: &str, options: NSActivityOptions) -> ActivityGuard {
        let process_info = NSProcessInfo::processInfo();
        let activity =
            process_info.beginActivityWithOptions_reason(options, &NSString::from_str(reason));
        #[cfg(test)]
        let release_observation = tests::observe_begin(&process_info, &activity, reason, options);
        let activity = Self {
            process_info,
            activity,
            #[cfg(test)]
            release_observation,
        };
        ActivityGuard::new(move || drop(activity))
    }
}

impl Drop for AppleActivity {
    fn drop(&mut self) {
        // SAFETY: This is the token created by this process-info instance, ended exactly once.
        unsafe { self.process_info.endActivity(&self.activity) };
        #[cfg(test)]
        tests::observe_end(
            &self.release_observation,
            &self.process_info,
            &self.activity,
        );
    }
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
