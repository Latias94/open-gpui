use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_core_foundation::CGRect;
use objc2_ui_kit::{UIWindow, UIWindowScene};
use std::{cell::Cell, rc::Rc};

define_class!(
    #[unsafe(super = UIWindow)]
    #[thread_kind = MainThreadOnly]
    #[name = "GPUIIosWindow"]
    #[ivars = Rc<Cell<bool>>]
    pub(super) struct NativeWindow;

    impl NativeWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            !self.ivars().get() && unsafe { msg_send![super(self), canBecomeKeyWindow] }
        }

        #[unsafe(method(makeKeyWindow))]
        fn make_key_window(&self) {
            if !self.ivars().get() {
                unsafe { let _: () = msg_send![super(self), makeKeyWindow]; }
            }
        }

        #[unsafe(method(becomeKeyWindow))]
        fn become_key_window(&self) {
            if !self.ivars().get() {
                unsafe { let _: () = msg_send![super(self), becomeKeyWindow]; }
            }
        }
    }
);

impl NativeWindow {
    #[allow(deprecated)]
    pub(super) fn new(
        frame: CGRect,
        scene: Option<&UIWindowScene>,
        interaction_quiesced: Rc<Cell<bool>>,
        main_thread: MainThreadMarker,
    ) -> Retained<UIWindow> {
        let this = Self::alloc(main_thread).set_ivars(interaction_quiesced);
        let this: Retained<Self> = if let Some(scene) = scene {
            unsafe { msg_send![super(this), initWithWindowScene: scene] }
        } else {
            unsafe { msg_send![super(this), initWithFrame: frame] }
        };
        this.setFrame(frame);
        this.into_super()
    }
}
