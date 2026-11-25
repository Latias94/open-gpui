//! Native desktop platforms and application constructors for Open GPUI.

#[cfg(all(
    any(target_os = "linux", target_os = "freebsd"),
    not(target_env = "ohos")
))]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(all(test, not(target_os = "windows")))]
#[path = "windows/color_glyph_bitmap.rs"]
mod color_glyph_bitmap;
#[cfg(all(test, not(any(target_os = "linux", target_os = "freebsd"))))]
#[path = "linux/power/profile.rs"]
mod power_profile;
#[cfg(all(test, not(any(target_os = "linux", target_os = "freebsd"))))]
#[path = "linux/wayland/frame_state.rs"]
mod wayland_frame_state;

pub use open_gpui::Platform;
pub use open_gpui_wgpu::SharedWgpuContext;

use std::rc::Rc;

/// Returns a background executor for the current platform.
pub fn background_executor() -> open_gpui::BackgroundExecutor {
    current_platform(true).background_executor()
}

pub fn application() -> open_gpui::Application {
    open_gpui::Application::with_platform(current_platform(false))
}

/// Creates a native application whose top-level windows all use the supplied WGPU context.
///
/// Create the instance without a bound display handle when GPUI opens the display connection.
/// The caller selects a device that can present to that display and owns device replacement.
pub fn application_with_shared_wgpu(
    context: SharedWgpuContext,
) -> anyhow::Result<open_gpui::Application> {
    #[cfg(target_os = "macos")]
    {
        Ok(open_gpui::Application::with_platform(Rc::new(
            macos::MacPlatform::new_with_shared_wgpu(context),
        )))
    }
    #[cfg(target_os = "windows")]
    {
        Ok(open_gpui::Application::with_platform(Rc::new(
            windows::WindowsPlatform::new_with_shared_wgpu(context)?,
        )))
    }
    #[cfg(all(
        any(target_os = "linux", target_os = "freebsd"),
        not(target_env = "ohos"),
        any(feature = "wayland", feature = "x11")
    ))]
    {
        Ok(open_gpui::Application::with_platform(
            linux::linux_platform_with_shared_wgpu(context),
        ))
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(
            any(target_os = "linux", target_os = "freebsd"),
            not(target_env = "ohos"),
            any(feature = "wayland", feature = "x11")
        )
    )))]
    {
        let _ = context;
        anyhow::bail!("no native WGPU windowing backend is enabled")
    }
}

pub fn headless() -> open_gpui::Application {
    open_gpui::Application::with_platform(current_platform(true))
}

/// Returns a Linux or FreeBSD app that may switch among `allowed_modes`.
///
/// It starts windowed in the process's own environment, or headless if that names no allowed
/// display server. Set another initial mode with [`open_gpui::Application::with_windowing`], and
/// switch later with [`open_gpui::App::request_windowing`]. Unlike [`headless`], this factory
/// retains the ability to attach to an allowed display server later.
/// To transfer an inherited Wayland socket, construct the platform with
/// `linux::linux_platform_with_startup` and pass it to
/// [`open_gpui::Application::with_platform`]. This factory does not adopt a Host descriptor
/// from `WAYLAND_SOCKET` or mutate the running Host's environment.
#[cfg(all(
    any(target_os = "linux", target_os = "freebsd"),
    not(target_env = "ohos")
))]
pub fn linux(allowed_modes: open_gpui::WindowingModes) -> open_gpui::Application {
    open_gpui::Application::with_platform(linux::linux_platform(allowed_modes))
}

/// Returns the default [`Platform`] for the current OS.
pub fn current_platform(headless: bool) -> Rc<dyn Platform> {
    #[cfg(target_os = "macos")]
    {
        Rc::new(macos::MacPlatform::new(headless))
    }

    #[cfg(target_os = "windows")]
    {
        Rc::new(
            windows::WindowsPlatform::new(headless).expect("failed to initialize Windows platform"),
        )
    }

    #[cfg(target_env = "ohos")]
    {
        let _ = headless;
        panic!(
            "open_gpui_platform does not select an OpenHarmony backend; inject a platform with open_gpui::Application::with_platform"
        )
    }

    #[cfg(all(
        any(target_os = "linux", target_os = "freebsd"),
        not(target_env = "ohos")
    ))]
    {
        linux::current_platform(headless)
    }
}

/// Returns a new [`open_gpui::PlatformHeadlessRenderer`] for the current platform, if available.
#[cfg(feature = "test-support")]
pub fn current_headless_renderer()
-> anyhow::Result<Option<Box<dyn open_gpui::PlatformHeadlessRenderer>>> {
    #[cfg(not(target_family = "wasm"))]
    {
        open_gpui_wgpu::WgpuHeadlessRenderer::new().map(|renderer| {
            Some(Box::new(renderer) as Box<dyn open_gpui::PlatformHeadlessRenderer>)
        })
    }

    #[cfg(target_family = "wasm")]
    {
        Ok(None)
    }
}

#[cfg(all(test, feature = "test-support", target_family = "wasm"))]
mod headless_facade_tests {
    #[test]
    fn unsupported_platform_has_no_default_headless_renderer() -> anyhow::Result<()> {
        assert!(super::current_headless_renderer()?.is_none());
        Ok(())
    }
}

#[cfg(all(test, target_os = "macos", feature = "test-support"))]
mod tests {
    use super::*;
    use open_gpui::{AppContext, Empty, VisualTestAppContext};
    use std::cell::RefCell;
    use std::time::Duration;

    // Note: All VisualTestAppContext tests are ignored by default because they require
    // the macOS main thread. Standard Rust tests run on worker threads, which causes
    // SIGABRT when interacting with macOS AppKit/Cocoa APIs.
    //
    // To run these tests, use:
    // cargo test -p open-gpui-platform --features test-support -- --ignored --test-threads=1

    #[test]
    #[ignore] // Requires macOS main thread
    fn test_foreground_tasks_run_with_run_until_parked() {
        let mut cx = VisualTestAppContext::new(current_platform(false));

        let task_ran = Rc::new(RefCell::new(false));

        // Spawn a foreground task via the App's spawn method
        // This should use our TestDispatcher, not the MacDispatcher
        {
            let task_ran = task_ran.clone();
            cx.update(|cx| {
                cx.spawn(async move |_| {
                    *task_ran.borrow_mut() = true;
                })
                .detach();
            });
        }

        // The task should not have run yet
        assert!(!*task_ran.borrow());

        // Run until parked should execute the foreground task
        cx.run_until_parked();

        // Now the task should have run
        assert!(*task_ran.borrow());
    }

    #[test]
    #[ignore] // Requires macOS main thread
    fn test_advance_clock_triggers_delayed_tasks() {
        let mut cx = VisualTestAppContext::new(current_platform(false));

        let task_ran = Rc::new(RefCell::new(false));

        // Spawn a task that waits for a timer
        {
            let task_ran = task_ran.clone();
            let executor = cx.background_executor.clone();
            cx.update(|cx| {
                cx.spawn(async move |_| {
                    executor.timer(Duration::from_millis(500)).await;
                    *task_ran.borrow_mut() = true;
                })
                .detach();
            });
        }

        // Run until parked - the task should be waiting on the timer
        cx.run_until_parked();
        assert!(!*task_ran.borrow());

        // Advance clock past the timer duration
        cx.advance_clock(Duration::from_millis(600));

        // Now the task should have completed
        assert!(*task_ran.borrow());
    }

    #[test]
    #[ignore] // Requires macOS main thread - window creation fails on test threads
    fn test_window_spawn_uses_test_dispatcher() {
        let mut cx = VisualTestAppContext::new(current_platform(false));

        let task_ran = Rc::new(RefCell::new(false));

        let window = cx
            .open_offscreen_window_default(|_, cx| cx.new(|_| Empty))
            .expect("Failed to open window");

        // Spawn a task via window.spawn - this is the critical test case
        // for tooltip behavior, as tooltips use window.spawn for delayed show
        {
            let task_ran = task_ran.clone();
            cx.update_window(window.into(), |_, window, cx| {
                window
                    .spawn(cx, async move |_| {
                        *task_ran.borrow_mut() = true;
                    })
                    .detach();
            })
            .ok();
        }

        // The task should not have run yet
        assert!(!*task_ran.borrow());

        // Run until parked should execute the foreground task spawned via window
        cx.run_until_parked();

        // Now the task should have run
        assert!(*task_ran.borrow());
    }
}
