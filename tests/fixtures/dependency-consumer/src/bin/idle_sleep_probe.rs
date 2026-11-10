//! Ordinary native App consumer of scoped activity ownership.

#[cfg(target_os = "windows")]
fn main() {
    use open_gpui::{Application, QuitMode};
    use std::{cell::RefCell, fs::File, io::Write as _, path::PathBuf, rc::Rc};
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

    fn handles() -> u32 {
        let mut count = 0;
        // SAFETY: The pseudo-handle names this process and the output is a live initialized u32.
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) }
            .expect("observe the isolated worker's native handles");
        count
    }

    let report_path = PathBuf::from(std::env::args_os().nth(1).expect("explicit report path"));
    let report = Rc::new(RefCell::new(None));
    Application::with_platform(open_gpui_platform::current_platform(true))
        .with_quit_mode(QuitMode::Explicit)
        .run(move |app| {
            assert!(!app.is_test());
            assert!(!app.background_executor().is_test());
            assert_eq!(app.platform_info().os, open_gpui::OperatingSystem::Windows);
            app.on_app_quit({
                let report = report.clone();
                move |_| {
                    let counts: [u32; 6] = report
                        .borrow_mut()
                        .take()
                        .expect("the public activity workflow must complete before native quit");
                    let mut file = File::create(&report_path).expect("create activity report");
                    writeln!(file, "native-activity-v1").unwrap();
                    for count in counts {
                        writeln!(file, "{count}").unwrap();
                    }
                    writeln!(file, "quit").unwrap();
                    file.sync_all()
                        .expect("persist native quit and activity observations");
                    std::future::ready(())
                }
            })
            .detach();

            let background = app.background_executor().clone();
            let warmed = background.spawn(async {});
            app.spawn(async move |cx| {
                warmed.await;
                background.timer(std::time::Duration::from_millis(20)).await;
                std::thread::spawn(|| {})
                    .join()
                    .expect("warm worker release");
                let baseline = handles();
                let mut guards = Vec::new();
                for index in 0..8 {
                    let request = cx.update(|app| {
                        app.prevent_idle_sleep(&format!("独立 activity scope {index}"))
                    });
                    guards.push(request.await.expect("acquire a real Windows activity"));
                }
                let eight = handles();
                assert_eq!(
                    eight,
                    baseline + 8,
                    "each native scope must own one request handle"
                );
                let remaining = guards.split_off(4);
                drop(guards);
                let four = handles();
                assert_eq!(
                    four,
                    baseline + 4,
                    "dropping one group must preserve the other group"
                );
                std::thread::spawn(move || drop(remaining))
                    .join()
                    .expect("release native activities from another thread");
                let released = handles();
                assert_eq!(released, baseline, "all request handles must be released");
                let app_nap = background.prevent_app_nap("ordinary background scope");
                let nap = handles();
                drop(app_nap);
                let nap_released = handles();
                assert_eq!(
                    nap, baseline,
                    "the non-Apple App Nap default owns no power request"
                );
                assert_eq!(nap_released, baseline);
                *report.borrow_mut() = Some([baseline, eight, four, released, nap, nap_released]);
                cx.update(|app| app.quit());
            })
            .detach();
        });
    panic!("the ordinary native App must terminate through its platform loop");
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("the ordinary activity worker is qualified on Windows only");
}
