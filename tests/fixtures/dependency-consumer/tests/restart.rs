//! Runs the public restart API in an isolated native worker with a dedicated recorder target.

#![cfg(target_os = "windows")]

use std::{
    ffi::OsString,
    fs,
    io::{Read, Write},
    os::windows::{ffi::OsStringExt as _, io::AsRawHandle as _, process::CommandExt as _},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        },
    },
    core::PCWSTR,
};

struct RestartJob(HANDLE);

impl RestartJob {
    fn new() -> Self {
        // SAFETY: No security attributes or name are borrowed; the returned handle is owned here.
        let job =
            Self(unsafe { CreateJobObjectW(None, PCWSTR::null()) }.expect("create restart job"));
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: The valid job receives a live, correctly sized information structure.
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )
        }
        .expect("own and retire restart descendants");
        job
    }

    fn assign(&self, child: &Child) {
        // SAFETY: Both handles remain live; the worker blocks on stdin before creating descendants.
        unsafe { AssignProcessToJobObject(self.0, HANDLE(child.as_raw_handle())) }
            .expect("assign the blocked native worker before release");
    }

    fn active_processes(&self) -> u32 {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: The live job writes only into the correctly sized initialized output.
        unsafe {
            QueryInformationJobObject(
                Some(self.0),
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut _,
                std::mem::size_of_val(&info) as u32,
                None,
            )
        }
        .expect("query restart descendants");
        info.ActiveProcesses
    }
}

impl Drop for RestartJob {
    fn drop(&mut self) {
        if self.active_processes() != 0 {
            // SAFETY: This handle owns only test descendants; termination is followed by observation.
            if let Err(error) = unsafe { TerminateJobObject(self.0, 1) } {
                eprintln!("restart descendant termination failed: {error}");
            }
            let started = Instant::now();
            while self.active_processes() != 0 && started.elapsed() < Duration::from_secs(5) {
                thread::sleep(Duration::from_millis(20));
            }
            if self.active_processes() != 0 {
                eprintln!("restart descendants did not retire within the cleanup deadline");
            }
        }
        // SAFETY: This owner closes its unique job handle, retiring only assigned test descendants.
        unsafe { CloseHandle(self.0) }.expect("close restart job");
    }
}

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
                    eprintln!("restart worker cleanup observation failed: {error}");
                    break;
                }
                Ok(None) if started.elapsed() >= Duration::from_secs(5) => {
                    eprintln!("restart worker did not retire within the cleanup deadline");
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
        let parent = fs::canonicalize(std::env::temp_dir()).expect("absolute temporary parent");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = parent.join(format!("open-gpui-restart-{}-{unique}", std::process::id()));
        assert_eq!(path.parent(), Some(parent.as_path()));
        fs::create_dir(&path).expect("create a new exclusively owned restart workspace");
        Self(fs::canonicalize(path).unwrap())
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        assert_eq!(
            self.0.parent(),
            fs::canonicalize(std::env::temp_dir()).ok().as_deref()
        );
        let started = Instant::now();
        loop {
            match fs::remove_dir_all(&self.0) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(error) if started.elapsed() >= Duration::from_secs(5) => {
                    eprintln!("restart workspace cleanup failed at {:?}: {error}", self.0);
                    break;
                }
                Err(_) => thread::sleep(Duration::from_millis(20)),
            }
        }
    }
}

fn read_u32(input: &mut impl Read) -> std::io::Result<u32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn recorded(path: &Path) -> std::io::Result<(u32, bool, Vec<OsString>)> {
    let mut file = fs::File::open(path)?;
    let mut header = [0; 9];
    file.read_exact(&mut header)?;
    assert_eq!(&header, b"OGPUIRST1");
    let pid = read_u32(&mut file)?;
    let mut flags = [0];
    file.read_exact(&mut flags)?;
    let count = read_u32(&mut file)?;
    assert!(count < 100);
    let mut args = Vec::new();
    for _ in 0..count {
        let length = read_u32(&mut file)?;
        assert!(length < 32768);
        let mut units = Vec::new();
        for _ in 0..length {
            let mut pair = [0; 2];
            file.read_exact(&mut pair)?;
            units.push(u16::from_le_bytes(pair));
        }
        args.push(OsString::from_wide(&units));
    }
    let mut trailing = [0];
    assert_eq!(file.read(&mut trailing)?, 0);
    Ok((pid, flags[0] != 0, args))
}

#[test]
fn downstream_native_restart_preserves_owned_argv_and_retires_only_its_worker() {
    run_restart(false);
}

#[test]
fn downstream_native_restart_launches_without_arguments_and_clears_transport() {
    run_restart(true);
}

#[test]
fn downstream_restart_job_cleanup_retires_a_blocked_worker() {
    let workspace = Workspace::new();
    let probe = env!("CARGO_BIN_EXE_restart_probe");
    let job = RestartJob::new();
    let mut child = Worker(
        Command::new(probe)
            .arg("--worker")
            .arg(workspace.0.join("unused-record.bin"))
            .arg(workspace.0.join("unused-lifecycle.txt"))
            .arg(probe)
            .arg("cleanup-control")
            .creation_flags(0x0800_0000)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start a blocked cleanup control"),
    );
    job.assign(&child.0);
    drop(job);
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the terminated Job's worker must become signaled before the cleanup deadline",
        );
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(1));
}

fn run_restart(empty: bool) {
    let workspace = Workspace::new();
    let target_dir = workspace.0.join("目标 $env ' tick` path");
    fs::create_dir(&target_dir).unwrap();
    let target = target_dir.join("restart $value ' 试验.exe");
    fs::copy(env!("CARGO_BIN_EXE_restart_probe"), &target).expect("copy the dedicated recorder");
    let record = workspace.0.join("record.bin");
    let lifecycle = workspace.0.join("lifecycle.txt");
    fs::write(&lifecycle, []).unwrap();
    let token = format!("restart-{}", std::process::id());
    let payload = vec![
        OsString::from(""),
        OsString::from("plain"),
        OsString::from("space and\ttab"),
        OsString::from("中文 参数"),
        OsString::from("a\"quoted\"value"),
        OsString::from("backslash\\\\\"quote"),
        OsString::from("trailing slash \\\\"),
        OsString::from("$env:USERNAME; $(literal) `tick 'quote & pipe|"),
        OsString::from_wide(&[0x61, 0xd800, 0x62]),
    ];
    let stdout = fs::File::create(workspace.0.join("stdout.log")).unwrap();
    let stderr_path = workspace.0.join("stderr.log");
    let stderr = fs::File::create(&stderr_path).unwrap();
    let job = RestartJob::new();
    let mut child = Worker(
        Command::new(env!("CARGO_BIN_EXE_restart_probe"))
            .arg(if empty { "--worker-empty" } else { "--worker" })
            .arg(&record)
            .arg(&lifecycle)
            .arg(&target)
            .arg(&token)
            .args(&payload)
            .env("OPEN_GPUI_RESTART_PID", "original-parent-value")
            .env("OPEN_GPUI_RESTART_EXECUTABLE", "original-parent-value")
            .env("OPEN_GPUI_RESTART_ARGUMENTS", "original-parent-value")
            .env("OPEN_GPUI_RESTART_PROBE_RECORD", &record)
            .creation_flags(0x0800_0000)
            .stdin(Stdio::piped())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .expect("start the blocked ordinary native worker"),
    );
    let worker_pid = child.0.id();
    job.assign(&child.0);
    child.0.stdin.take().unwrap().write_all(&[1]).unwrap();
    let started = Instant::now();
    loop {
        let status = child.0.try_wait().unwrap();
        if status.is_some() && recorded(&record).is_ok() && job.active_processes() == 0 {
            assert!(status.unwrap().success());
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "native restart did not complete: worker={status:?}, descendants={}, lifecycle={:?}, stderr={:?}",
            job.active_processes(),
            fs::read_to_string(&lifecycle),
            fs::read_to_string(&stderr_path),
        );
        thread::sleep(Duration::from_millis(20));
    }
    let (recorder_pid, transport_present, args) = recorded(&record).unwrap();
    assert_ne!(worker_pid, recorder_pid);
    assert!(
        !transport_present,
        "the temporary launcher environment must be cleared"
    );
    assert_eq!(
        fs::canonicalize(PathBuf::from(&args[0])).unwrap(),
        fs::canonicalize(&target).unwrap(),
        "the recorder must use the explicitly selected executable",
    );
    assert_ne!(
        fs::canonicalize(PathBuf::from(&args[0])).unwrap(),
        fs::canonicalize(env!("CARGO_BIN_EXE_restart_probe")).unwrap(),
        "falling back to the worker executable must not pass",
    );
    let mut expected = Vec::new();
    if !empty {
        expected.extend([
            OsString::from("--record"),
            record.into_os_string(),
            OsString::from(token),
            OsString::from(worker_pid.to_string()),
        ]);
        expected.extend(payload);
    }
    assert_eq!(
        &args[1..],
        expected,
        "the actual launcher must preserve each native argv value"
    );
    assert_eq!(
        fs::read_to_string(lifecycle).unwrap(),
        "launch\nrestart\nquit\n"
    );
    assert_eq!(job.active_processes(), 0);
}
