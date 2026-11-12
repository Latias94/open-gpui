//! Real AppKit termination probe, run in a fresh macOS GUI process with:
//! `cargo run -p open-gpui-macos --example native_termination`.
//!
//! The quit callback first refuses completion, allows queued foreground work to run, then confirms
//! shutdown. AppKit must deliver its real termination notification and exit successfully. This
//! low-level probe checks a host borrow and the platform lock; the workbench covers Core App's
//! shutdown fence. No window, simulated reply, or NSApplication::stop is involved.

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(not(target_os = "macos"))]
fn main() -> std::process::ExitCode {
    eprintln!("native_termination requires macOS; no native termination evidence");
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "macos")]
mod macos {
    use block2::RcBlock;
    use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSString};
    use open_gpui::{ActivationPolicy, Platform};
    use open_gpui_macos::MacPlatform;
    use std::{
        cell::RefCell,
        io::Write as _,
        ptr::NonNull,
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    fn require(condition: bool, message: &str) {
        if !condition {
            eprintln!("NATIVE TERMINATION FAILED: {message}");
            std::process::exit(1);
        }
    }

    pub(super) fn run() {
        let _main_thread = MainThreadMarker::new()
            .expect("native termination must start on the process main thread");
        let attempts = Arc::new(AtomicUsize::new(0));
        let foreground_ran = Arc::new(AtomicBool::new(false));
        let allowed = Arc::new(AtomicBool::new(false));
        let native_terminal = Arc::new(AtomicBool::new(false));
        std::thread::spawn({
            let attempts = attempts.clone();
            let foreground_ran = foreground_ran.clone();
            let native_terminal = native_terminal.clone();
            move || {
                std::thread::sleep(Duration::from_secs(5));
                eprintln!(
                    "NATIVE TERMINATION FAILED: process remained alive after 5s; callbacks={}, queued_work={}, native_terminal={}",
                    attempts.load(Ordering::SeqCst),
                    foreground_ran.load(Ordering::SeqCst),
                    native_terminal.load(Ordering::SeqCst),
                );
                std::process::exit(1);
            }
        });

        objc2::rc::autoreleasepool(|_| {
            let platform = Rc::new(MacPlatform::new(false));
            platform.set_activation_policy(ActivationPolicy::Accessory);
            let host = Rc::new(RefCell::new(()));

            let center = NSNotificationCenter::defaultCenter();
            let notification = NSString::from_str("NSApplicationWillTerminateNotification");
            let block = RcBlock::new({
                let attempts = attempts.clone();
                let foreground_ran = foreground_ran.clone();
                let allowed = allowed.clone();
                let native_terminal = native_terminal.clone();
                move |_: NonNull<NSNotification>| {
                    require(
                        MainThreadMarker::new().is_some(),
                        "termination notification was not on the main thread",
                    );
                    require(
                        attempts.load(Ordering::SeqCst) == 2
                            && foreground_ran.load(Ordering::SeqCst)
                            && allowed.load(Ordering::SeqCst),
                        "AppKit terminated before the original quit callback confirmed shutdown",
                    );
                    require(
                        !native_terminal.swap(true, Ordering::SeqCst),
                        "AppKit delivered termination more than once",
                    );
                    println!(
                        "NATIVE TERMINATION: real AppKit terminal observed; false→queued foreground work→true; repeated quit coalesced; awaiting actual process exit"
                    );
                    require(
                        std::io::stdout().flush().is_ok(),
                        "cannot flush native evidence",
                    );
                }
            });
            let _observer: Retained<AnyObject> = unsafe {
                objc2::msg_send![&*center,
                    addObserverForName: &*notification,
                    object: None::<&AnyObject>,
                    queue: None::<&AnyObject>,
                    usingBlock: &*block
                ]
            };

            platform.on_quit(Box::new({
                let platform = Rc::downgrade(&platform);
                let host = host.clone();
                let attempts = attempts.clone();
                let foreground_ran = foreground_ran.clone();
                let allowed = allowed.clone();
                move || {
                    require(MainThreadMarker::new().is_some(), "quit callback left the main thread");
                    let borrow = host.try_borrow_mut();
                    require(borrow.is_ok(), "quit callback ran inside the requesting host borrow");
                    let _borrow = borrow;
                    let platform = platform.upgrade().expect("the running platform is retained");
                    // Both getters lock the actual MacPlatformState. A callback holding that lock
                    // deadlocks here, and the independent watchdog reports a failing process.
                    let foreground = platform.foreground_executor();
                    require(platform.background_executor().is_main_thread(), "wrong native executor");
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    match attempt {
                        1 => {
                            for _ in 0..4 {
                                platform.quit();
                            }
                            foreground.spawn({
                                let platform = Rc::downgrade(&platform);
                                let host = host.clone();
                                let foreground_ran = foreground_ran.clone();
                                async move {
                                    require(
                                        host.try_borrow_mut().is_ok(),
                                        "queued work ran before the quit callback released its host borrow",
                                    );
                                    let platform = platform.upgrade().expect("running platform");
                                    for _ in 0..4 {
                                        platform.quit();
                                    }
                                    foreground_ran.store(true, Ordering::SeqCst);
                                }
                            }).detach();
                            false
                        }
                        2 => {
                            require(
                                foreground_ran.load(Ordering::SeqCst),
                                "TerminateLater blocked the foreground shutdown continuation",
                            );
                            allowed.store(true, Ordering::SeqCst);
                            true
                        }
                        _ => {
                            require(false, "repeated quit created a second termination handshake");
                            false
                        }
                    }
                }
            }));

            platform.run(Box::new({
                let platform = platform.clone();
                let host = host.clone();
                let attempts = attempts.clone();
                move || {
                    let _borrow = host.borrow_mut();
                    for _ in 0..4 {
                        platform.quit();
                    }
                    require(
                        attempts.load(Ordering::SeqCst) == 0,
                        "quit synchronously entered the callback while the host was borrowed",
                    );
                }
            }));
            require(false, "AppKit run returned without terminating the process");
        });
    }
}
