//! Metal frame requests use a per-window link on macOS 14 and later.
//! CoreVideo fallback frame requests use one immortal link per display.
//!
//! `CVDisplayLinkStop` does not wait for its output callback. Callback contexts
//! therefore contain only integer display ids, and the registry retains every
//! created link. Window subscriptions and their signals are removed under the
//! registry lock before their dispatch sources can be cancelled or released.
//! CoreVideo start and stop calls run outside that lock on the main thread.

use block2::RcBlock;
use core_graphics::display::CGDirectDisplayID;

use dispatch2::{
    _dispatch_source_type_data_add, DispatchObject, DispatchQueue, DispatchRetained, DispatchSource,
};
use open_gpui::{FrameRequestSource, PlatformFrameSignal};
pub(crate) use open_gpui_apple::display_link::{
    MetalFrameGeneration, MetalFrameSource, MetalFrames,
};
use open_gpui_scheduler::Instant;
use std::{
    collections::{BTreeMap, btree_map},
    error::Error,
    ffi::c_void,
    fmt,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

static REGISTRY: Mutex<Registry> = Mutex::new(Registry::new());

struct Registry {
    displays: BTreeMap<CGDirectDisplayID, DisplayEntry>,
    next_subscriber_id: u64,
}

impl Registry {
    const fn new() -> Self {
        Self {
            displays: BTreeMap::new(),
            next_subscriber_id: 0,
        }
    }
}

struct DisplayEntry {
    link: sys::DisplayLink,
    running: bool,
    subscribers: Vec<DisplayLinkSubscriber>,
}

struct DisplayLinkSubscriber {
    id: SubscriberId,
    frame_requests: DispatchRetained<DispatchSource>,
    signal: Arc<PlatformFrameSignal>,
    pending: Arc<AtomicBool>,
}

// SAFETY: The CoreVideo and GCD handles are thread-safe refcounted objects. The
// registry lock serializes all entry access, and the display thread only records
// signals and merges dispatch data through subscribers retained by that lock.
unsafe impl Send for DisplayEntry {}

#[derive(Clone, Copy, PartialEq, Eq)]
struct SubscriberId(u64);

fn lock_registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

fn debug_assert_main_thread() {
    #[cfg(debug_assertions)]
    {
        debug_assert!(
            open_gpui_apple::main_thread_marker().is_some(),
            "display link registry mutations must happen on the main thread"
        );
    }
}

unsafe extern "C" fn display_link_output_callback(
    _display_link_out: *mut sys::CVDisplayLink,
    current_time: *const sys::CVTimeStamp,
    _output_time: *const sys::CVTimeStamp,
    _flags_in: i64,
    _flags_out: *mut i64,
    display_id: *mut c_void,
) -> i32 {
    let signal_at = PlatformFrameSignal::capture(|| {
        let current_host_time = objc2_core_video::CVGetCurrentHostTime();
        let received_at = Instant::now();
        let host_clock_frequency = objc2_core_video::CVGetHostClockFrequency();
        // SAFETY: CoreVideo owns this timestamp for the lifetime of the callback.
        let current_time = unsafe { current_time.as_ref() };
        frame_request_instant(
            current_time,
            current_host_time,
            host_clock_frequency,
            received_at,
        )
    });
    let display_id = display_id as usize as CGDirectDisplayID;
    let registry = lock_registry();
    if let Some(entry) = registry.displays.get(&display_id) {
        for subscriber in &entry.subscribers {
            if let Some(signal_at) = signal_at {
                subscriber
                    .signal
                    .record(signal_at, FrameRequestSource::NativeCallback);
            }
            subscriber.pending.store(true, Ordering::Release);
            subscriber.frame_requests.merge_data(1);
        }
    }
    0
}

/// Maps a CoreVideo host timestamp using a paired clock sample.
///
/// Invalid timestamps or clock conversions fall back to callback receipt time.
fn frame_request_instant(
    timestamp: Option<&sys::CVTimeStamp>,
    current_host_time: u64,
    host_clock_frequency: f64,
    received_at: Instant,
) -> Instant {
    let Some(timestamp) = timestamp.filter(|timestamp| {
        timestamp.version == 0 && timestamp.flags & sys::kCVTimeStampHostTimeValid != 0
    }) else {
        return received_at;
    };
    if !host_clock_frequency.is_finite() || host_clock_frequency <= 0.0 {
        return received_at;
    }
    current_host_time
        .checked_sub(timestamp.host_time)
        .and_then(|elapsed_ticks| {
            Duration::try_from_secs_f64(elapsed_ticks as f64 / host_clock_frequency).ok()
        })
        .and_then(|elapsed| received_at.checked_sub(elapsed))
        .unwrap_or(received_at)
}

fn subscribe(
    display_id: CGDirectDisplayID,
    frame_requests: DispatchRetained<DispatchSource>,
    signal: Arc<PlatformFrameSignal>,
    pending: Arc<AtomicBool>,
) -> Result<SubscriberId, DisplayLinkError> {
    debug_assert_main_thread();

    let needs_link = !lock_registry().displays.contains_key(&display_id);
    let new_link = if needs_link {
        // SAFETY: The callback context is an integer display id; every callback
        // accesses only the immortal registry, including callbacks after stop.
        Some(unsafe {
            sys::DisplayLink::new(
                display_id,
                display_link_output_callback,
                display_id as usize as *mut c_void,
            )?
        })
    } else {
        None
    };

    let (subscriber_id, link_to_start) = {
        let mut registry = lock_registry();
        let registry = &mut *registry;
        let subscriber_id = SubscriberId(registry.next_subscriber_id);
        registry.next_subscriber_id += 1;
        let entry = match (registry.displays.entry(display_id), new_link) {
            (btree_map::Entry::Occupied(entry), _) => entry.into_mut(),
            (btree_map::Entry::Vacant(vacant), Some(link)) => vacant.insert(DisplayEntry {
                link,
                running: false,
                subscribers: Vec::new(),
            }),
            (btree_map::Entry::Vacant(_), None) => {
                return Err(DisplayLinkError::MissingDisplayLink);
            }
        };
        entry.subscribers.push(DisplayLinkSubscriber {
            id: subscriber_id,
            frame_requests,
            signal,
            pending,
        });
        let link_to_start = if entry.running {
            None
        } else {
            entry.running = true;
            Some(entry.link.clone())
        };
        (subscriber_id, link_to_start)
    };

    if let Some(mut link) = link_to_start {
        // SAFETY: Registry mutations are main-thread-only. CoreVideo calls run
        // outside the registry lock to avoid a cycle with its callback thread.
        if let Err(error) = unsafe { link.start() } {
            let mut registry = lock_registry();
            if let Some(entry) = registry.displays.get_mut(&display_id) {
                entry.running = false;
                entry.subscribers.retain(|subscriber| {
                    if subscriber.id == subscriber_id {
                        subscriber.signal.take();
                        false
                    } else {
                        true
                    }
                });
            }
            return Err(error);
        }
    }

    Ok(subscriber_id)
}

fn unsubscribe(display_id: CGDirectDisplayID, subscriber_id: SubscriberId) {
    debug_assert_main_thread();

    let link_to_stop = {
        let mut registry = lock_registry();
        let Some(entry) = registry.displays.get_mut(&display_id) else {
            return;
        };
        entry
            .subscribers
            .retain(|subscriber| subscriber.id != subscriber_id);
        if entry.subscribers.is_empty() && entry.running {
            entry.running = false;
            Some(entry.link.clone())
        } else {
            None
        }
    };

    if let Some(mut link) = link_to_stop {
        // SAFETY: The immortal link and integer context outlive straggler
        // callbacks, which can no longer reach the removed subscriber.
        if let Err(error) = unsafe { link.stop() }
            && !error.is_not_running()
        {
            log::warn!("{error}");
        }
    }
}

const K_CV_RETURN_INVALID_ARGUMENT: i32 = -6661;
const K_CV_RETURN_UNSUPPORTED: i32 = -6663;
const K_CV_RETURN_DISPLAY_LINK_NOT_RUNNING: i32 = -6672;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisplayLinkError {
    Create { code: i32 },
    SetOutputCallback { code: i32 },
    AssignDisplay { code: i32 },
    Start { code: i32 },
    Stop { code: i32 },
    MissingDisplayLink,
}

impl DisplayLinkError {
    pub(crate) fn is_transient_create_failure(self) -> bool {
        matches!(
            self,
            Self::Create {
                code: K_CV_RETURN_INVALID_ARGUMENT | K_CV_RETURN_UNSUPPORTED
            }
        )
    }

    fn is_not_running(self) -> bool {
        matches!(
            self,
            Self::Stop {
                code: K_CV_RETURN_DISPLAY_LINK_NOT_RUNNING
            }
        )
    }
}

impl fmt::Display for DisplayLinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create { code } => {
                write!(formatter, "could not create display link, code: {code}")
            }
            Self::SetOutputCallback { code } => {
                write!(
                    formatter,
                    "could not set display link output callback, code: {code}"
                )
            }
            Self::AssignDisplay { code } => {
                write!(
                    formatter,
                    "could not assign display to display link, code: {code}"
                )
            }
            Self::Start { code } => {
                write!(formatter, "could not start display link, code: {code}")
            }
            Self::Stop { code } => {
                write!(formatter, "could not stop display link, code: {code}")
            }
            Self::MissingDisplayLink => formatter.write_str("display link handle is missing"),
        }
    }
}

impl Error for DisplayLinkError {}

/// A per-window dispatch source subscribed to its display's immortal link.
/// Its handler holds only a weak window lease and is retired after cancellation.
pub struct WindowFrameSource {
    frame_requests: DispatchRetained<DispatchSource>,
    registration: Option<(CGDirectDisplayID, SubscriberId)>,
    signal: Arc<PlatformFrameSignal>,
    pending: Arc<AtomicBool>,
}

impl WindowFrameSource {
    pub fn new(callback: impl Fn() + Send + Sync + 'static) -> Self {
        let pending = Arc::new(AtomicBool::new(false));
        let handler: RcBlock<dyn Fn()> = RcBlock::new({
            let pending = pending.clone();
            move || {
                if pending.swap(false, Ordering::AcqRel) {
                    callback();
                }
            }
        });
        // SAFETY: GCD copies the block and invokes it on the main queue. It
        // owns only a weak window lease and synchronized pending state, so a
        // queued event cannot dereference a retired native view.
        let frame_requests = unsafe {
            let frame_requests = DispatchSource::new(
                &raw const _dispatch_source_type_data_add as *mut _,
                0,
                0,
                Some(DispatchQueue::main()),
            );
            frame_requests.set_event_handler_with_block(RcBlock::as_ptr(&handler));
            frame_requests.resume();
            frame_requests
        };
        Self {
            frame_requests,
            registration: None,
            signal: Arc::new(PlatformFrameSignal::new()),
            pending,
        }
    }

    pub fn start(&mut self, display_id: CGDirectDisplayID) -> Result<(), DisplayLinkError> {
        self.stop();
        let subscriber_id = subscribe(
            display_id,
            self.frame_requests.clone(),
            self.signal.clone(),
            self.pending.clone(),
        )?;
        self.registration = Some((display_id, subscriber_id));
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.registration.is_some()
    }

    /// Returns the current subscription without consuming a pending frame signal.
    pub fn debug_state(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": "CVDisplayLink",
            "attached": self.registration.is_some(),
            "display_id": self.registration.map(|(display_id, _)| display_id),
        })
    }

    pub fn take_signal(&self) -> Option<(Instant, FrameRequestSource)> {
        self.signal.take()
    }

    pub fn stop(&mut self) {
        if let Some((display_id, subscriber_id)) = self.registration.take() {
            unsubscribe(display_id, subscriber_id);
        }
        self.signal.take();
        self.pending.store(false, Ordering::Release);
    }
}

impl Drop for WindowFrameSource {
    fn drop(&mut self) {
        self.stop();
        self.frame_requests.cancel();
    }
}

mod sys {
    //! Derived from display-link crate under the following license:
    //! <https://github.com/BrainiumLLC/display-link/blob/master/LICENSE-MIT>
    //! Apple docs: [CVDisplayLink](https://developer.apple.com/documentation/corevideo/cvdisplaylinkoutputcallback?language=objc)
    #![allow(dead_code, non_upper_case_globals)]

    use super::{DisplayLinkError, K_CV_RETURN_INVALID_ARGUMENT};
    use core_graphics::display::CGDirectDisplayID;
    use foreign_types::{ForeignType, foreign_type};
    use std::{
        ffi::c_void,
        fmt::{self, Debug, Formatter},
    };

    #[derive(Debug)]
    pub enum CVDisplayLink {}

    foreign_type! {
        pub unsafe type DisplayLink {
            type CType = CVDisplayLink;
            fn drop = CVDisplayLinkRelease;
            fn clone = CVDisplayLinkRetain;
        }
    }

    impl Debug for DisplayLink {
        fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
            formatter
                .debug_tuple("DisplayLink")
                .field(&self.as_ptr())
                .finish()
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub(crate) struct CVTimeStamp {
        pub version: u32,
        pub video_time_scale: i32,
        pub video_time: i64,
        pub host_time: u64,
        pub rate_scalar: f64,
        pub video_refresh_period: i64,
        pub smpte_time: CVSMPTETime,
        pub flags: u64,
        pub reserved: u64,
    }

    pub type CVTimeStampFlags = u64;

    pub const kCVTimeStampVideoTimeValid: CVTimeStampFlags = 1 << 0;
    pub const kCVTimeStampHostTimeValid: CVTimeStampFlags = 1 << 1;
    pub const kCVTimeStampSMPTETimeValid: CVTimeStampFlags = 1 << 2;
    pub const kCVTimeStampVideoRefreshPeriodValid: CVTimeStampFlags = 1 << 3;
    pub const kCVTimeStampRateScalarValid: CVTimeStampFlags = 1 << 4;
    pub const kCVTimeStampTopField: CVTimeStampFlags = 1 << 16;
    pub const kCVTimeStampBottomField: CVTimeStampFlags = 1 << 17;
    pub const kCVTimeStampVideoHostTimeValid: CVTimeStampFlags =
        kCVTimeStampVideoTimeValid | kCVTimeStampHostTimeValid;
    pub const kCVTimeStampIsInterlaced: CVTimeStampFlags =
        kCVTimeStampTopField | kCVTimeStampBottomField;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub(crate) struct CVSMPTETime {
        pub subframes: i16,
        pub subframe_divisor: i16,
        pub counter: u32,
        pub time_type: u32,
        pub flags: u32,
        pub hours: i16,
        pub minutes: i16,
        pub seconds: i16,
        pub frames: i16,
    }

    pub type CVSMPTETimeType = u32;

    pub const kCVSMPTETimeType24: CVSMPTETimeType = 0;
    pub const kCVSMPTETimeType25: CVSMPTETimeType = 1;
    pub const kCVSMPTETimeType30Drop: CVSMPTETimeType = 2;
    pub const kCVSMPTETimeType30: CVSMPTETimeType = 3;
    pub const kCVSMPTETimeType2997: CVSMPTETimeType = 4;
    pub const kCVSMPTETimeType2997Drop: CVSMPTETimeType = 5;
    pub const kCVSMPTETimeType60: CVSMPTETimeType = 6;
    pub const kCVSMPTETimeType5994: CVSMPTETimeType = 7;

    pub type CVSMPTETimeFlags = u32;

    pub const kCVSMPTETimeValid: CVSMPTETimeFlags = 1 << 0;
    pub const kCVSMPTETimeRunning: CVSMPTETimeFlags = 1 << 1;

    pub type CVDisplayLinkOutputCallback = unsafe extern "C" fn(
        display_link_out: *mut CVDisplayLink,
        // A pointer to the current timestamp. This represents the timestamp when the callback is called.
        current_time: *const CVTimeStamp,
        // A pointer to the output timestamp. This represents the timestamp for when the frame will be displayed.
        output_time: *const CVTimeStamp,
        // Unused
        flags_in: i64,
        // Unused
        flags_out: *mut i64,
        // A pointer to app-defined data.
        display_link_context: *mut c_void,
    ) -> i32;

    #[link(name = "CoreFoundation", kind = "framework")]
    #[link(name = "CoreVideo", kind = "framework")]
    #[allow(improper_ctypes, unknown_lints, clippy::duplicated_attributes)]
    unsafe extern "C" {
        pub fn CVDisplayLinkCreateWithActiveCGDisplays(
            display_link_out: *mut *mut CVDisplayLink,
        ) -> i32;
        pub fn CVDisplayLinkSetCurrentCGDisplay(
            display_link: &mut DisplayLinkRef,
            display_id: u32,
        ) -> i32;
        pub fn CVDisplayLinkSetOutputCallback(
            display_link: &mut DisplayLinkRef,
            callback: CVDisplayLinkOutputCallback,
            user_info: *mut c_void,
        ) -> i32;
        pub fn CVDisplayLinkStart(display_link: &mut DisplayLinkRef) -> i32;
        pub fn CVDisplayLinkStop(display_link: &mut DisplayLinkRef) -> i32;
        pub fn CVDisplayLinkRelease(display_link: *mut CVDisplayLink);
        pub fn CVDisplayLinkRetain(display_link: *mut CVDisplayLink) -> *mut CVDisplayLink;
    }

    impl DisplayLink {
        /// Apple docs: [CVDisplayLinkCreateWithCGDisplay](https://developer.apple.com/documentation/corevideo/1456981-cvdisplaylinkcreatewithcgdisplay?language=objc)
        pub unsafe fn new(
            display_id: CGDirectDisplayID,
            callback: CVDisplayLinkOutputCallback,
            user_info: *mut c_void,
        ) -> Result<Self, DisplayLinkError> {
            unsafe {
                let mut display_link: *mut CVDisplayLink = 0 as _;

                let code = CVDisplayLinkCreateWithActiveCGDisplays(&mut display_link);
                if code != 0 {
                    return Err(DisplayLinkError::Create { code });
                }
                if display_link.is_null() {
                    return Err(DisplayLinkError::Create {
                        code: K_CV_RETURN_INVALID_ARGUMENT,
                    });
                }

                let mut display_link = DisplayLink::from_ptr(display_link);

                let code = CVDisplayLinkSetOutputCallback(&mut display_link, callback, user_info);
                if code != 0 {
                    return Err(DisplayLinkError::SetOutputCallback { code });
                }

                let code = CVDisplayLinkSetCurrentCGDisplay(&mut display_link, display_id);
                if code != 0 {
                    return Err(DisplayLinkError::AssignDisplay { code });
                }

                Ok(display_link)
            }
        }
    }

    impl DisplayLinkRef {
        /// Apple docs: [CVDisplayLinkStart](https://developer.apple.com/documentation/corevideo/1457193-cvdisplaylinkstart?language=objc)
        pub unsafe fn start(&mut self) -> Result<(), DisplayLinkError> {
            unsafe {
                let code = CVDisplayLinkStart(self);
                if code != 0 {
                    return Err(DisplayLinkError::Start { code });
                }
                Ok(())
            }
        }

        /// Apple docs: [CVDisplayLinkStop](https://developer.apple.com/documentation/corevideo/1457281-cvdisplaylinkstop?language=objc)
        pub unsafe fn stop(&mut self) -> Result<(), DisplayLinkError> {
            unsafe {
                let code = CVDisplayLinkStop(self);
                if code != 0 {
                    return Err(DisplayLinkError::Stop { code });
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_timestamp_preserves_request_age_at_different_clock_rates() {
        let received_at = Instant::now();
        for (host_time, current_host_time, frequency, age) in [
            (4_000_000, 10_000_000, 1e9, Duration::from_millis(6)),
            (1_760_000, 2_000_000, 24e6, Duration::from_millis(10)),
        ] {
            let timestamp = timestamp(host_time);
            assert_eq!(
                frame_request_instant(Some(&timestamp), current_host_time, frequency, received_at),
                received_at - age,
            );
        }
    }

    #[test]
    fn invalid_host_timestamps_fall_back_to_receipt_time() {
        let received_at = Instant::now();
        let valid = timestamp(1_000);
        let mut missing_host_time = valid;
        missing_host_time.flags = 0;
        let mut unknown_version = valid;
        unknown_version.version = 1;
        for timestamp in [None, Some(&missing_host_time), Some(&unknown_version)] {
            assert_eq!(
                frame_request_instant(timestamp, 2_000, 1e9, received_at),
                received_at,
            );
        }
        assert_eq!(
            frame_request_instant(Some(&valid), 999, 1e9, received_at),
            received_at,
        );
        for frequency in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MIN_POSITIVE] {
            assert_eq!(
                frame_request_instant(Some(&valid), 2_000, frequency, received_at),
                received_at,
            );
        }
    }

    fn timestamp(host_time: u64) -> sys::CVTimeStamp {
        sys::CVTimeStamp {
            host_time,
            flags: sys::kCVTimeStampHostTimeValid,
            ..Default::default()
        }
    }
}
