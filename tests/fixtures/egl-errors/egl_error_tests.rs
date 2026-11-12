use khronos_egl as egl;
use std::sync::Mutex;

static TEST_LOCK: Mutex<()> = Mutex::new(());

type Binding = egl::DynamicInstance<egl::EGL1_4>;
type ErrorReads = unsafe extern "system" fn() -> usize;

#[derive(Clone, Copy, Debug)]
enum Call {
    Initialize,
    WindowSurface,
    PlatformWindowSurface,
}

impl Call {
    fn invoke(self, instance: &Binding) -> Result<(), egl::Error> {
        let mut display_handle = 0u8;
        let mut config_handle = 0u8;
        let mut window_handle = 0u8;
        let display =
            unsafe { egl::Display::from_ptr(std::ptr::from_mut(&mut display_handle).cast()) };
        let config =
            unsafe { egl::Config::from_ptr(std::ptr::from_mut(&mut config_handle).cast()) };
        let window = std::ptr::from_mut(&mut window_handle).cast();

        match self {
            Self::Initialize => instance.initialize(display).map(|version| {
                assert_eq!(version, (1, 5));
            }),
            Self::WindowSurface => unsafe {
                instance.create_window_surface(display, config, window, Some(&[egl::NONE]))
            }
            .map(|surface| assert!(!surface.as_ptr().is_null())),
            Self::PlatformWindowSurface => unsafe {
                instance
                    .upcast::<egl::EGL1_5>()
                    .expect("mock must export EGL 1.5")
                    .create_platform_window_surface(display, config, window, &[egl::ATTRIB_NONE])
            }
            .map(|surface| assert!(!surface.as_ptr().is_null())),
        }
    }
}

const CALLS: [Call; 3] = [
    Call::Initialize,
    Call::WindowSurface,
    Call::PlatformWindowSurface,
];

fn with_instance(result: egl::Boolean, error: egl::Int, run: impl FnOnce(&Binding, ErrorReads)) {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let path =
        std::env::var_os("OPEN_GPUI_EGL_MOCK").expect("set OPEN_GPUI_EGL_MOCK to the mock library");
    let library = unsafe { libloading::Library::new(&path) }.expect("load mock controls");
    let set_result = unsafe {
        library.get::<unsafe extern "system" fn(egl::Boolean, egl::Int)>(b"mockSetResult\0")
    }
    .expect("load result control");
    let error_reads =
        unsafe { library.get::<ErrorReads>(b"mockErrorReads\0") }.expect("load error-read counter");
    let instance = unsafe { Binding::load_required_from_filename(&path) }
        .expect("load actual EGL 1.4 binding from mock symbols");
    assert_eq!(instance.version(), egl::Version::EGL1_5);
    unsafe { set_result(result, error) };
    run(&instance, *error_reads);
}

#[test]
fn failed_calls_with_success_return_unreported() {
    for call in CALLS {
        with_instance(egl::FALSE, egl::SUCCESS, |instance, error_reads| {
            let error = call.invoke(instance).expect_err("failed native call");
            assert_eq!(error.native(), egl::SUCCESS, "{call:?}");
            assert_eq!(
                error.to_string(),
                "An EGL call failed, but eglGetError returned EGL_SUCCESS.",
                "{call:?}"
            );
            assert_eq!(unsafe { error_reads() }, 1, "{call:?}");
            assert_eq!(instance.get_error(), None, "{call:?}");
        });
    }
}

#[test]
fn failed_calls_preserve_real_errors() {
    for call in CALLS {
        for expected in [egl::Error::BadNativeWindow, egl::Error::BadMatch] {
            with_instance(egl::FALSE, expected.native(), |instance, error_reads| {
                assert_eq!(call.invoke(instance), Err(expected), "{call:?}");
                assert_eq!(unsafe { error_reads() }, 1, "{call:?}");
                assert_eq!(instance.get_error(), None, "{call:?}");
            });
        }
    }
}

#[test]
fn successful_calls_do_not_query_error() {
    for call in CALLS {
        with_instance(egl::TRUE, egl::BAD_MATCH, |instance, error_reads| {
            assert_eq!(call.invoke(instance), Ok(()), "{call:?}");
            assert_eq!(unsafe { error_reads() }, 0, "{call:?}");
            assert_eq!(instance.get_error(), Some(egl::Error::BadMatch), "{call:?}");
        });
    }
}
