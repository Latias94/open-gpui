//! Ordinary native restart worker and argv recorder. The owning test supplies the target.

#[cfg(target_os = "windows")]
fn main() -> std::io::Result<()> {
    use std::{
        ffi::OsString,
        fs::{File, OpenOptions},
        io::{Read, Write},
        os::windows::ffi::OsStrExt as _,
        path::PathBuf,
    };

    fn append(path: &std::path::Path, event: &[u8]) {
        let mut output = OpenOptions::new()
            .append(true)
            .open(path)
            .expect("lifecycle log");
        output.write_all(event).expect("write lifecycle event");
        output.sync_all().expect("persist lifecycle event");
    }

    let args = std::env::args_os().collect::<Vec<_>>();
    let empty_recorder = (args.len() == 1)
        .then(|| std::env::var_os("OPEN_GPUI_RESTART_PROBE_RECORD"))
        .flatten();
    if args.get(1).is_some_and(|role| role == "--record") || empty_recorder.is_some() {
        if empty_recorder.is_none() {
            use windows::Win32::{
                Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
                System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
            };
            let worker_pid = args
                .get(4)
                .and_then(|value| value.to_str())
                .and_then(|value| value.parse::<u32>().ok())
                .expect("original worker PID");
            // SAFETY: Only the explicitly supplied test worker is observed; the handle is closed.
            match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, worker_pid) } {
                Ok(worker) => {
                    // SAFETY: The live synchronization handle is polled without waiting or mutation.
                    let status = unsafe { WaitForSingleObject(worker, 0) };
                    // SAFETY: This scope owns the handle returned by OpenProcess.
                    unsafe { CloseHandle(worker) }.expect("close original-worker observation");
                    assert_eq!(
                        status, WAIT_OBJECT_0,
                        "the recorder must start after worker exit"
                    );
                }
                Err(error) => assert_eq!(
                    error.code(),
                    ERROR_INVALID_PARAMETER.to_hresult(),
                    "only a retired PID may be absent",
                ),
            }
        }
        let path = empty_recorder
            .as_ref()
            .or_else(|| args.get(2))
            .expect("the recorder requires its explicit output");
        let mut output = File::create(path)?;
        output.write_all(b"OGPUIRST1")?;
        output.write_all(&std::process::id().to_le_bytes())?;
        let transport_present = [
            "OPEN_GPUI_RESTART_PID",
            "OPEN_GPUI_RESTART_EXECUTABLE",
            "OPEN_GPUI_RESTART_ARGUMENTS",
        ]
        .iter()
        .any(|key| std::env::var_os(key).is_some());
        output.write_all(&[u8::from(transport_present)])?;
        output.write_all(&(args.len() as u32).to_le_bytes())?;
        for argument in args {
            let native = argument.encode_wide().collect::<Vec<_>>();
            output.write_all(&(native.len() as u32).to_le_bytes())?;
            for code_unit in native {
                output.write_all(&code_unit.to_le_bytes())?;
            }
        }
        output.sync_all()?;
        return Ok(());
    }

    let empty = args.get(1).is_some_and(|role| role == "--worker-empty");
    assert!(empty || args.get(1).is_some_and(|role| role == "--worker"));
    let record = args.get(2).expect("explicit recorder output").clone();
    let lifecycle = PathBuf::from(args.get(3).expect("explicit lifecycle log"));
    let target = PathBuf::from(args.get(4).expect("explicit restart target"));
    let token = args.get(5).expect("explicit token").clone();
    assert!(target.is_absolute() && target.is_file());
    let mut release = [0];
    std::io::stdin().read_exact(&mut release)?;
    assert_eq!(release, [1], "the parent releases only a job-owned worker");

    let mut restart_arguments = vec![
        OsString::from("--record"),
        record,
        token,
        OsString::from(std::process::id().to_string()),
    ];
    restart_arguments.extend(args.into_iter().skip(6));
    if empty {
        restart_arguments.clear();
    }
    let application =
        open_gpui::Application::with_platform(open_gpui_platform::current_platform(true))
            .with_quit_mode(open_gpui::QuitMode::Explicit)
            .with_restart_arguments(restart_arguments);
    application.run(move |app| {
        assert!(!app.is_test());
        assert!(!app.background_executor().is_test());
        assert_eq!(app.platform_info().os, open_gpui::OperatingSystem::Windows);
        append(&lifecycle, b"launch\n");
        let restarting = lifecycle.clone();
        app.on_app_restart(move |_| append(&restarting, b"restart\n"))
            .detach();
        app.on_app_quit(move |app| {
            let lifecycle = lifecycle.clone();
            let timer = app
                .background_executor()
                .timer(std::time::Duration::from_millis(20));
            async move {
                timer.await;
                append(&lifecycle, b"quit\n");
            }
        })
        .detach();
        app.set_restart_path(target);
        app.restart();
    });
    panic!("the native worker must terminate through the platform run loop");
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("the ordinary native restart recorder is qualified on Windows only");
}
