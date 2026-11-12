use super::*;
use objc2::Message as _;

pub(super) const OWNER_IVAR: &CStr = c"nativeCloseOwner";
const RETRY_DELAY: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeClosePhase {
    Open,
    Closing,
    Closed,
}

struct NativeCloseOwner {
    authority: Arc<Mutex<MacPresentationShutdownAuthority>>,
    foreground: ForegroundExecutor,
    background: BackgroundExecutor,
    phase: Cell<NativeClosePhase>,
}

struct NativeCloseRequest {
    window: Retained<AnyObject>,
    window_state: Arc<Mutex<MacWindowState>>,
    shutdown: WindowPresentationShutdownTicket,
}

impl NativeCloseOwner {
    fn request(self: &Rc<Self>, window: &AnyObject) {
        if self.phase.get() != NativeClosePhase::Open {
            return;
        }
        self.phase.set(NativeClosePhase::Closing);
        let shutdown = self.authority.lock().ticket();
        let request = NativeCloseRequest {
            window: window.retain(),
            window_state: unsafe { get_window_state(window) },
            shutdown,
        };
        if self.try_close(&request) {
            return;
        }
        let snapshot = request.shutdown.snapshot();
        if snapshot.protocol_violation() || snapshot.detached() {
            log::error!(
                "native macOS close retains an invalid presentation shutdown for window {:?}, generation {}: {snapshot:?}",
                snapshot.window_id(),
                snapshot.generation(),
            );
        } else {
            log::debug!(
                "deferring native macOS close until presentation quiescence for window {:?}, generation {}",
                snapshot.window_id(),
                snapshot.generation(),
            );
        }
        let owner = self.clone();
        self.foreground
            .spawn(async move {
                loop {
                    owner.background.timer(RETRY_DELAY).await;
                    if owner.try_close(&request) {
                        break;
                    }
                }
            })
            .detach();
    }

    fn try_close(&self, request: &NativeCloseRequest) -> bool {
        if quiesce_presentation_surface(&request.window_state, &request.shutdown)
            != PlatformPresentationShutdownOutcome::Quiesced
        {
            return false;
        }
        if !finish_native_close(&request.window_state, &request.window, &request.shutdown) {
            return false;
        }
        self.phase.set(NativeClosePhase::Closed);
        true
    }
}

pub(super) unsafe fn install(window: &AnyObject, platform_window: &MacWindow) {
    let (foreground, background) = {
        let state = platform_window.0.lock();
        (
            state.foreground_executor.clone(),
            state.background_executor.clone(),
        )
    };
    let owner = Rc::new(NativeCloseOwner {
        authority: platform_window.1.clone(),
        foreground,
        background,
        phase: Cell::new(NativeClosePhase::Open),
    });
    unsafe {
        window
            .class()
            .instance_variable(OWNER_IVAR)
            .expect("GPUI native close owner ivar")
            .load_ptr::<*mut c_void>(window)
            .write(Rc::into_raw(owner).cast_mut().cast());
    }
}

pub(super) unsafe fn request(window: &AnyObject) {
    unsafe {
        let raw = *window
            .class()
            .instance_variable(OWNER_IVAR)
            .expect("GPUI native close owner ivar")
            .load::<*mut c_void>(window);
        if raw.is_null() {
            let state = *window
                .class()
                .instance_variable(WINDOW_STATE_IVAR)
                .expect("GPUI window state ivar")
                .load::<*mut c_void>(window);
            if state.is_null() {
                let superclass = window_callback_superclass(window);
                let _: () = msg_send![super(window, superclass), close];
            } else {
                log::error!("native macOS close requested without an installed close owner");
            }
            return;
        }
        let owner = mem::ManuallyDrop::new(Rc::from_raw(raw.cast::<NativeCloseOwner>()));
        let owner = Rc::clone(&owner);
        owner.request(window);
    }
}

pub(super) unsafe fn drop_owner(window: &AnyObject) {
    unsafe {
        let slot = window
            .class()
            .instance_variable(OWNER_IVAR)
            .expect("GPUI native close owner ivar")
            .load_ptr::<*mut c_void>(window);
        let raw = slot.replace(ptr::null_mut());
        if !raw.is_null() {
            drop(Rc::from_raw(raw.cast::<NativeCloseOwner>()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_close_reuses_the_prepared_authority() {
        let window_id = WindowId::from(7);
        let mut authority = MacPresentationShutdownAuthority::new(window_id);
        let candidate = WindowPresentationShutdownTicket::new(window_id, 5);
        let prepared = authority.claim(candidate.clone()).unwrap();
        let native = authority.ticket();

        assert!(native.same_authority(&candidate));
        assert!(native.acknowledge_quiesced());
        assert!(native.acknowledge_native_terminal());
        let snapshot = prepared.snapshot();
        assert!(snapshot.quiesced());
        assert!(snapshot.native_terminal());
        assert!(!snapshot.protocol_violation());
    }

    #[test]
    fn later_core_preparation_reuses_the_native_close_authority() {
        let window_id = WindowId::from(7);
        let mut authority = MacPresentationShutdownAuthority::new(window_id);
        let native = authority.ticket();
        let prepared = authority
            .claim(WindowPresentationShutdownTicket::new(window_id, 9))
            .unwrap();

        assert!(native.same_authority(&prepared));
        assert!(native.acknowledge_quiesced());
        assert!(native.acknowledge_native_terminal());
        let snapshot = prepared.snapshot();
        assert!(snapshot.quiesced());
        assert!(snapshot.native_terminal());
        assert!(!snapshot.protocol_violation());
    }
}
