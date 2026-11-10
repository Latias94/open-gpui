//! Ordinary consumers of the official activity APIs without a test-platform dependency.

use open_gpui::{ActivityGuard, App, BackgroundExecutor, Platform, PlatformDispatcher, Task};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn downstream_activity_contracts_are_available_without_optional_features() {
    let _: fn(&App, &str) -> Task<open_gpui::Result<ActivityGuard>> = App::prevent_idle_sleep;
    let _: fn(&dyn Platform, &str) -> Task<open_gpui::Result<ActivityGuard>> =
        <dyn Platform>::prevent_idle_sleep;
    let _: fn(&BackgroundExecutor, &str) -> ActivityGuard = BackgroundExecutor::prevent_app_nap;
    let _: fn(&(dyn PlatformDispatcher + 'static), &str) -> ActivityGuard =
        <dyn PlatformDispatcher>::prevent_app_nap;
    fn assert_send<T: Send>() {}
    assert_send::<ActivityGuard>();
}

#[test]
fn downstream_owned_activity_releases_once_after_thread_transfer_and_unwind() {
    let released = Arc::new(AtomicUsize::new(0));
    let first = ActivityGuard::new({
        let released = released.clone();
        move || {
            released.fetch_add(1, Ordering::SeqCst);
        }
    });
    let second = ActivityGuard::new({
        let released = released.clone();
        move || {
            released.fetch_add(10, Ordering::SeqCst);
        }
    });
    drop(ActivityGuard::noop());
    assert_eq!(released.load(Ordering::SeqCst), 0);
    std::thread::spawn(move || drop(first)).join().unwrap();
    assert_eq!(released.load(Ordering::SeqCst), 1);
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _second = second;
        panic!("injected downstream activity failure");
    }));
    assert!(failure.is_err());
    assert_eq!(released.load(Ordering::SeqCst), 11);
}

#[cfg(target_os = "windows")]
mod native {
    use std::{
        fs,
        os::windows::process::CommandExt as _,
        path::PathBuf,
        process::{Child, Command},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    struct Worker(Child);

    impl Drop for Worker {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
            }
            let started = Instant::now();
            loop {
                match self.0.try_wait() {
                    Ok(Some(_)) => break,
                    Err(error) => {
                        eprintln!("activity worker cleanup failed: {error}");
                        break;
                    }
                    Ok(None) if started.elapsed() >= Duration::from_secs(5) => {
                        eprintln!("activity worker did not retire before cleanup deadline");
                        break;
                    }
                    Ok(None) => thread::sleep(Duration::from_millis(20)),
                }
            }
        }
    }

    struct Workspace(PathBuf);

    impl Workspace {
        fn new() -> Self {
            let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = parent.join(format!(
                "open-gpui-activity-{}-{unique}",
                std::process::id()
            ));
            assert_eq!(root.parent(), Some(parent.as_path()));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for Workspace {
        fn drop(&mut self) {
            for name in ["report.txt", "stdout.log", "stderr.log"] {
                let _ = fs::remove_file(self.0.join(name));
            }
            if let Err(error) = fs::remove_dir(&self.0) {
                eprintln!("activity workspace cleanup failed: {error}");
            }
        }
    }

    #[test]
    fn downstream_native_app_releases_scoped_power_handles_before_quit() {
        let workspace = Workspace::new();
        let report = workspace.0.join("report.txt");
        let stderr = workspace.0.join("stderr.log");
        let mut worker = Worker(
            Command::new(env!("CARGO_BIN_EXE_idle_sleep_probe"))
                .arg(&report)
                .creation_flags(0x0800_0000)
                .stdout(fs::File::create(workspace.0.join("stdout.log")).unwrap())
                .stderr(fs::File::create(&stderr).unwrap())
                .spawn()
                .expect("start only the dedicated ordinary activity worker"),
        );
        let started = Instant::now();
        let status = loop {
            if let Some(status) = worker.0.try_wait().unwrap() {
                break status;
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "ordinary native activity worker timed out: {:?}",
                fs::read_to_string(&stderr),
            );
            thread::sleep(Duration::from_millis(20));
        };
        assert!(
            status.success(),
            "native worker failed: {:?}",
            fs::read_to_string(&stderr)
        );
        let report = fs::read_to_string(report).expect("completed native App quit report");
        let mut lines = report.lines();
        assert_eq!(lines.next(), Some("native-activity-v1"));
        let counts = lines
            .by_ref()
            .take(6)
            .map(|line| line.parse::<u32>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(counts.len(), 6);
        assert_eq!(counts[1], counts[0] + 8);
        assert_eq!(counts[2], counts[0] + 4);
        assert_eq!(&counts[3..], &[counts[0]; 3]);
        assert_eq!(lines.next(), Some("quit"));
        assert_eq!(lines.next(), None);
    }
}
