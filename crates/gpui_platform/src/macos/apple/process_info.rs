use block2::RcBlock;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSNotificationName, NSObjectProtocol, NSProcessInfo,
    NSProcessInfoThermalState, NSProcessInfoThermalStateDidChangeNotification, NSString,
};
use open_gpui::{NativeAppNotificationHandler, PowerMode, ThermalState};
use parking_lot::Mutex;
use std::{ptr::NonNull, sync::Arc};

pub fn thermal_state() -> ThermalState {
    match NSProcessInfo::processInfo().thermalState() {
        NSProcessInfoThermalState::Fair => ThermalState::Fair,
        NSProcessInfoThermalState::Serious => ThermalState::Serious,
        NSProcessInfoThermalState::Critical => ThermalState::Critical,
        _ => ThermalState::Nominal,
    }
}

/// Foundation reports Low Power Mode but cannot distinguish Balanced from HighPower.
pub fn power_mode() -> PowerMode {
    let process_info = NSProcessInfo::processInfo();
    if process_info.respondsToSelector(objc2::sel!(isLowPowerModeEnabled))
        && process_info.isLowPowerModeEnabled()
    {
        PowerMode::LowPower
    } else {
        PowerMode::Unknown
    }
}

pub struct ProcessInfoObserver {
    center: Retained<NSNotificationCenter>,
    observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    callback: Arc<Mutex<Option<NativeAppNotificationHandler>>>,
}

impl ProcessInfoObserver {
    pub fn thermal(callback: NativeAppNotificationHandler) -> Self {
        // Foundation requires reading thermalState before registering for changes.
        // https://developer.apple.com/documentation/foundation/processinfo/thermalstatedidchangenotification
        thermal_state();
        Self::with_center(
            NSNotificationCenter::defaultCenter(),
            unsafe { NSProcessInfoThermalStateDidChangeNotification },
            callback,
        )
    }

    pub fn power(callback: NativeAppNotificationHandler) -> Self {
        let name = NSString::from_str("NSProcessInfoPowerStateDidChangeNotification");
        Self::with_center(NSNotificationCenter::defaultCenter(), &name, callback)
    }

    fn with_center(
        center: Retained<NSNotificationCenter>,
        name: &NSNotificationName,
        callback: NativeAppNotificationHandler,
    ) -> Self {
        let callback = Arc::new(Mutex::new(Some(callback)));
        let notification_callback = callback.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            let callback = notification_callback.lock().clone();
            if let Some(callback) = callback {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback()));
                if let Err(panic) = result {
                    std::mem::forget(panic);
                    log::error!("isolated a panic from an Apple process-info notification");
                }
            }
        });
        // SAFETY: The block captures only synchronized owned state. The handler
        // is a Send + Sync App ingress producer; application work is deferred by
        // that ingress, even for synchronous or off-thread Foundation posts.
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(name),
                Some(&NSProcessInfo::processInfo()),
                None,
                &block,
            )
        };
        Self {
            center,
            observer,
            callback,
        }
    }
}

impl Drop for ProcessInfoObserver {
    fn drop(&mut self) {
        let callback = self.callback.lock().take();
        // SAFETY: This token belongs to this center and is removed exactly once.
        // An already admitted producer owns its handler until ingress returns.
        unsafe { self.center.removeObserver(self.observer.as_ref()) };
        drop(callback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn independent_process_observers_retire_without_delegate_or_run_loop() {
        for notification_name in [
            "NSProcessInfoPowerStateDidChangeNotification",
            "NSProcessInfoThermalStateDidChangeNotification",
        ] {
            let name = NSString::from_str(notification_name);
            let center = NSNotificationCenter::new();
            let calls = Arc::new(AtomicUsize::new(0));
            let callback: NativeAppNotificationHandler = Arc::new({
                let calls = calls.clone();
                move || {
                    calls.fetch_add(1, Ordering::Relaxed);
                }
            });
            let first = ProcessInfoObserver::with_center(center.clone(), &name, callback.clone());
            let second = ProcessInfoObserver::with_center(center.clone(), &name, callback);
            let post = || unsafe {
                center.postNotificationName_object(&name, Some(&NSProcessInfo::processInfo()));
            };
            post();
            assert_eq!(calls.load(Ordering::Relaxed), 2);
            std::thread::scope(|scope| {
                let center = center.clone();
                scope
                    .spawn(move || {
                        objc2::rc::autoreleasepool(|_| unsafe {
                            center.postNotificationName_object(
                                &NSString::from_str(notification_name),
                                Some(&NSProcessInfo::processInfo()),
                            );
                        });
                    })
                    .join()
                    .unwrap();
            });
            assert_eq!(calls.load(Ordering::Relaxed), 4);
            drop(first);
            post();
            assert_eq!(calls.load(Ordering::Relaxed), 5);
            drop(second);
            post();
            assert_eq!(calls.load(Ordering::Relaxed), 5);
        }
    }
}
