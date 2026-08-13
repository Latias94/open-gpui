use super::{
    NATIVE_SCENARIO_ENV, NativeScenarioRegistration, inject_primary_button_up_best_effort,
};
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    fs::File,
    io::{BufRead as _, BufReader, Read as _, Seek as _, SeekFrom, Write as _},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HWND, POINT},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        },
        Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        },
    },
    UI::Input::KeyboardAndMouse::GetCapture,
    UI::WindowsAndMessaging::{
        GetCursorPos, GetForegroundWindow, IsWindow, SetCursorPos, SetForegroundWindow,
    },
};

const WORKER_SCENARIO_ENV: &str = "OPEN_GPUI_NATIVE_DOCK_WORKER";
const WORKER_PROTOCOL_NONCE_ENV: &str = "OPEN_GPUI_NATIVE_DOCK_PROTOCOL_NONCE";
const WORKER_REPORT_SCHEMA_VERSION: u32 = 3;
const WORKER_PROCESS_TIMEOUT: Duration = Duration::from_secs(60);
const WORKER_EXIT_AFTER_EOF_TIMEOUT: Duration = Duration::from_secs(2);
const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(20);
const LOG_TAIL_BYTES: usize = 32 * 1024;
const WORKER_START_COMMAND: &str = "START";
const WORKER_RELEASE_COMMAND: &str = "RELEASE";
const WORKER_REPORT_PREFIX: &str = "OPEN_GPUI_NATIVE_DOCK_REPORT";

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct NativeWorkerReport {
    schema_version: u32,
    scenario_id: String,
    outcome: NativeWorkerOutcome,
    milestones: Vec<String>,
    census: NativeWorkerCensus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "status", content = "message", rename_all = "snake_case")]
pub(super) enum NativeWorkerOutcome {
    Passed,
    Failed(String),
}

enum WorkerOutputEvent {
    Report(Result<NativeWorkerReport, String>),
    Eof,
    ReadFailure(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct NativeProcessWindowCensus {
    top_level_hwnds: Vec<isize>,
    message_only_hwnds: Vec<isize>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct NativeWorkerAppCensus {
    pub(super) window_registry_count: usize,
    pub(super) active_drag: bool,
    pub(super) native_exit_authority_settled: bool,
    pub(super) surface_session_closed: bool,
    pub(super) surface_runtime_empty: Option<bool>,
    pub(super) pending_terminal_ticket_count: usize,
    pub(super) failed_terminal_ticket_count: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct NativeWorkerCensus {
    before_application: NativeProcessWindowCensus,
    after_application: NativeProcessWindowCensus,
    app: NativeWorkerAppCensus,
    capture_released: bool,
    observed_native_generations: usize,
    terminal_native_generations: usize,
    unterminated_native_generations: Vec<String>,
}

impl NativeWorkerReport {
    fn passed(scenario_id: &str, milestones: Vec<String>, census: NativeWorkerCensus) -> Self {
        Self {
            schema_version: WORKER_REPORT_SCHEMA_VERSION,
            scenario_id: scenario_id.to_owned(),
            outcome: NativeWorkerOutcome::Passed,
            milestones,
            census,
        }
    }

    fn failed(scenario_id: &str, message: String, census: NativeWorkerCensus) -> Self {
        Self {
            schema_version: WORKER_REPORT_SCHEMA_VERSION,
            scenario_id: scenario_id.to_owned(),
            outcome: NativeWorkerOutcome::Failed(message),
            milestones: Vec::new(),
            census,
        }
    }
}

pub(super) fn is_worker(scenario_id: &str) -> bool {
    env::var(WORKER_SCENARIO_ENV)
        .ok()
        .is_some_and(|worker| worker == scenario_id)
}

pub(super) fn await_worker_start() {
    let nonce = worker_protocol_nonce();
    let command = read_worker_command();
    if command != format!("{WORKER_START_COMMAND} {nonce}") {
        eprintln!("native Dock worker received an invalid start command");
        std::process::exit(2);
    }
}

pub(super) fn capture_process_window_census() -> Result<NativeProcessWindowCensus> {
    process_window_census(std::process::id())
}

pub(super) fn finish_worker_report(
    scenario_id: &str,
    outcome: NativeWorkerOutcome,
    milestones: Vec<String>,
    before_application: NativeProcessWindowCensus,
    app: NativeWorkerAppCensus,
    observed_native_generations: usize,
    terminal_native_generations: usize,
    unterminated_native_generations: Vec<String>,
) -> Result<NativeWorkerReport> {
    let census = NativeWorkerCensus {
        before_application,
        after_application: capture_process_window_census()?,
        app,
        capture_released: unsafe { GetCapture() } == HWND::default(),
        observed_native_generations,
        terminal_native_generations,
        unterminated_native_generations,
    };
    Ok(match outcome {
        NativeWorkerOutcome::Passed => NativeWorkerReport::passed(scenario_id, milestones, census),
        NativeWorkerOutcome::Failed(message) => {
            NativeWorkerReport::failed(scenario_id, message, census)
        }
    })
}

pub(super) fn publish_worker_report_and_wait_for_release(report: &NativeWorkerReport) {
    let nonce = worker_protocol_nonce();
    let body = serde_json::to_string(report).unwrap_or_else(|error| {
        eprintln!("native Dock worker could not serialize its report: {error}");
        std::process::exit(2);
    });
    let mut stdout = std::io::stdout().lock();
    if let Err(error) =
        writeln!(stdout, "{WORKER_REPORT_PREFIX} {nonce} {body}").and_then(|()| stdout.flush())
    {
        eprintln!("native Dock worker could not publish its report: {error}");
        std::process::exit(2);
    }

    let command = read_worker_command();
    if command != format!("{WORKER_RELEASE_COMMAND} {nonce}") {
        eprintln!("native Dock worker received an invalid release command");
        std::process::exit(2);
    }
}

fn worker_protocol_nonce() -> String {
    env::var(WORKER_PROTOCOL_NONCE_ENV).unwrap_or_else(|_| {
        eprintln!("native Dock worker is missing {WORKER_PROTOCOL_NONCE_ENV}");
        std::process::exit(2);
    })
}

fn read_worker_command() -> String {
    let mut command = String::new();
    if let Err(error) = std::io::stdin().read_line(&mut command) {
        eprintln!("native Dock worker could not read its parent command: {error}");
        std::process::exit(2);
    }
    command.trim_end_matches(['\r', '\n']).to_owned()
}

pub(super) fn run_case_in_worker(registration: &NativeScenarioRegistration) {
    let scenario_id = registration.id.as_str();
    let _desktop = ParentDesktopState::capture();
    let artifacts = WorkerArtifacts::new(scenario_id);
    let stderr = File::create(&artifacts.stderr_path).unwrap_or_else(|error| {
        panic!(
            "scenario `{}` could not create worker stderr `{}`: {error}",
            scenario_id,
            artifacts.stderr_path.display()
        )
    });
    let executable = env::current_exe().expect("native Dock parent must resolve its test binary");
    let child = Command::new(&executable)
        .arg("--exact")
        .arg(&registration.test)
        .arg("--ignored")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(NATIVE_SCENARIO_ENV, scenario_id)
        .env(WORKER_SCENARIO_ENV, scenario_id)
        .env(WORKER_PROTOCOL_NONCE_ENV, &artifacts.protocol_nonce)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr))
        .spawn()
        .unwrap_or_else(|error| {
            panic!(
                "scenario `{}` could not start native worker `{}`: {error}",
                scenario_id,
                executable.display()
            )
        });
    let job = WorkerJob::new().unwrap_or_else(|error| {
        panic!("scenario `{scenario_id}` could not create its worker job: {error:#}")
    });
    job.assign_process(child.id()).unwrap_or_else(|error| {
        panic!("scenario `{scenario_id}` could not bind its worker to the job: {error:#}")
    });
    let mut child = WorkerChild::new(child, job, &artifacts);
    child.send_command(WORKER_START_COMMAND, &artifacts);
    let report = child.wait_for_report(scenario_id, &artifacts);
    assert_eq!(
        report.schema_version,
        WORKER_REPORT_SCHEMA_VERSION,
        "scenario `{}` worker report schema drifted\n{}",
        scenario_id,
        artifacts.summary()
    );
    assert_eq!(
        report.scenario_id,
        scenario_id,
        "scenario `{}` worker reported a different scenario id\n{}",
        scenario_id,
        artifacts.summary()
    );
    assert_worker_census(scenario_id, child.id(), &report.census, &artifacts);
    assert_eq!(
        child.job_active_processes(),
        1,
        "scenario `{scenario_id}` retained an unexpected live descendant before release\n{}",
        artifacts.summary()
    );
    child.send_command(WORKER_RELEASE_COMMAND, &artifacts);
    let status = child.wait_for_exit(scenario_id, &artifacts);
    assert!(
        status.success(),
        "scenario `{}` worker exited unsuccessfully after the parent accepted its pre-exit census: {status}\n{}",
        scenario_id,
        artifacts.summary()
    );
    assert_eq!(
        child.job_active_processes(),
        0,
        "scenario `{scenario_id}` left a process alive in its worker job\n{}",
        artifacts.summary()
    );
    match report.outcome {
        NativeWorkerOutcome::Passed => {
            assert!(
                report
                    .milestones
                    .iter()
                    .any(|milestone| milestone == "scenario.completed"),
                "scenario `{}` worker omitted its completion milestone: {:?}\n{}",
                scenario_id,
                report.milestones,
                artifacts.summary()
            );
        }
        NativeWorkerOutcome::Failed(message) => {
            panic!(
                "scenario `{}` worker reported failure: {message}\n{}",
                scenario_id,
                artifacts.summary()
            );
        }
    }
    artifacts.remove_logs_after_success();
}

fn assert_worker_census(
    scenario_id: &str,
    worker_pid: u32,
    census: &NativeWorkerCensus,
    artifacts: &WorkerArtifacts,
) {
    assert_eq!(
        census.after_application,
        census.before_application,
        "scenario `{}` did not restore the worker's exact pre-application HWND census before process exit\n{}",
        scenario_id,
        artifacts.summary()
    );
    let parent_observation = process_window_census(worker_pid).unwrap_or_else(|error| {
        panic!(
            "scenario `{}` parent could not establish the live pre-exit HWND census: {error:#}\n{}",
            scenario_id,
            artifacts.summary()
        )
    });
    assert_eq!(
        parent_observation,
        census.after_application,
        "scenario `{}` parent and worker disagree about the live pre-exit HWND census\n{}",
        scenario_id,
        artifacts.summary()
    );
    assert!(
        census.capture_released,
        "scenario `{}` returned from the application with native capture still owned\n{}",
        scenario_id,
        artifacts.summary()
    );
    assert!(
        census.unterminated_native_generations.is_empty()
            && census.observed_native_generations == census.terminal_native_generations,
        "scenario `{}` returned with native generations lacking terminal observations: observed={}, terminal={}, unterminated={:?}\n{}",
        scenario_id,
        census.observed_native_generations,
        census.terminal_native_generations,
        census.unterminated_native_generations,
        artifacts.summary()
    );
    assert_eq!(
        census.app.window_registry_count,
        0,
        "scenario `{scenario_id}` returned with logical windows still registered\n{}",
        artifacts.summary()
    );
    assert!(
        !census.app.active_drag,
        "scenario `{scenario_id}` returned with an active payload drag\n{}",
        artifacts.summary()
    );
    assert!(
        census.app.native_exit_authority_settled,
        "scenario `{scenario_id}` returned before the application native-exit authority settled\n{}",
        artifacts.summary()
    );
    assert!(
        census.app.surface_session_closed,
        "scenario `{scenario_id}` returned before its DockSurface window session closed\n{}",
        artifacts.summary()
    );
    assert_eq!(
        census.app.surface_runtime_empty,
        Some(true),
        "scenario `{scenario_id}` returned before its DockSurface runtime emptied\n{}",
        artifacts.summary()
    );
    assert_eq!(
        census.app.pending_terminal_ticket_count,
        0,
        "scenario `{scenario_id}` returned with pending terminal tickets\n{}",
        artifacts.summary()
    );
    assert_eq!(
        census.app.failed_terminal_ticket_count,
        0,
        "scenario `{scenario_id}` returned with failed terminal tickets\n{}",
        artifacts.summary()
    );
}

fn process_window_census(process_id: u32) -> Result<NativeProcessWindowCensus> {
    let census = open_gpui_windows::native_test_process_window_census(process_id)?;
    Ok(NativeProcessWindowCensus {
        top_level_hwnds: census.top_level_hwnds().to_owned(),
        message_only_hwnds: census.message_only_hwnds().to_owned(),
    })
}

struct ParentDesktopState {
    cursor: Option<POINT>,
    foreground: HWND,
}

impl ParentDesktopState {
    fn capture() -> Self {
        let mut cursor = POINT::default();
        let cursor = unsafe { GetCursorPos(&mut cursor) }
            .is_ok()
            .then_some(cursor);
        Self {
            cursor,
            foreground: unsafe { GetForegroundWindow() },
        }
    }
}

impl Drop for ParentDesktopState {
    fn drop(&mut self) {
        let _ = inject_primary_button_up_best_effort();
        if let Some(cursor) = self.cursor {
            let _ = unsafe { SetCursorPos(cursor.x, cursor.y) };
        }
        if self.foreground != HWND::default()
            && unsafe { IsWindow(Some(self.foreground)).as_bool() }
        {
            let _ = unsafe { SetForegroundWindow(self.foreground) };
        }
    }
}

struct WorkerArtifacts {
    protocol_nonce: String,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl WorkerArtifacts {
    fn new(scenario_id: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();
        let stem = format!(
            "open-gpui-native-dock-{}-{}-{nonce}",
            std::process::id(),
            scenario_id.replace('.', "-")
        );
        let directory = env::temp_dir();
        Self {
            protocol_nonce: format!("{}-{nonce:x}", std::process::id()),
            stdout_path: directory.join(format!("{stem}.stdout.log")),
            stderr_path: directory.join(format!("{stem}.stderr.log")),
        }
    }

    fn summary(&self) -> String {
        format!(
            "worker stdout log: {}\nworker stderr log: {}\nworker stdout tail:\n{}\nworker stderr tail:\n{}",
            self.stdout_path.display(),
            self.stderr_path.display(),
            read_log_tail(&self.stdout_path),
            read_log_tail(&self.stderr_path)
        )
    }

    fn remove_logs_after_success(&self) {
        let _ = fs::remove_file(&self.stdout_path);
        let _ = fs::remove_file(&self.stderr_path);
    }
}

struct WorkerChild {
    child: Option<Child>,
    job: WorkerJob,
    stdin: Option<ChildStdin>,
    report: Receiver<WorkerOutputEvent>,
    output_reader: Option<JoinHandle<()>>,
    protocol_nonce: String,
}

impl WorkerChild {
    fn new(mut child: Child, job: WorkerJob, artifacts: &WorkerArtifacts) -> Self {
        let stdin = child
            .stdin
            .take()
            .expect("piped native worker stdin must be available");
        let stdout = child
            .stdout
            .take()
            .expect("piped native worker stdout must be available");
        let (report_sender, report) = mpsc::channel();
        let protocol_nonce = artifacts.protocol_nonce.clone();
        let output_reader = spawn_worker_output_reader(
            stdout,
            artifacts.stdout_path.clone(),
            protocol_nonce.clone(),
            report_sender,
        );
        Self {
            child: Some(child),
            job,
            stdin: Some(stdin),
            report,
            output_reader: Some(output_reader),
            protocol_nonce,
        }
    }

    fn id(&self) -> u32 {
        self.child.as_ref().expect("worker child must be live").id()
    }

    fn send_command(&mut self, command: &str, artifacts: &WorkerArtifacts) {
        let stdin = self
            .stdin
            .as_mut()
            .expect("native worker protocol must remain writable");
        writeln!(stdin, "{command} {}", self.protocol_nonce)
            .and_then(|()| stdin.flush())
            .unwrap_or_else(|error| {
                panic!(
                    "native worker protocol could not send `{command}`: {error}\n{}",
                    artifacts.summary()
                )
            });
    }

    fn wait_for_report(
        &mut self,
        scenario_id: &str,
        artifacts: &WorkerArtifacts,
    ) -> NativeWorkerReport {
        let deadline = Instant::now() + WORKER_PROCESS_TIMEOUT;
        loop {
            match self.report.recv_timeout(WORKER_POLL_INTERVAL) {
                Ok(WorkerOutputEvent::Report(Ok(report))) => return report,
                Ok(WorkerOutputEvent::Report(Err(error))) => {
                    panic!(
                        "scenario `{scenario_id}` worker report protocol failed: {error}\n{}",
                        artifacts.summary()
                    )
                }
                Ok(WorkerOutputEvent::ReadFailure(error)) => {
                    panic!(
                        "scenario `{scenario_id}` worker stdout failed before its report: {error}\n{}",
                        artifacts.summary()
                    )
                }
                Ok(WorkerOutputEvent::Eof) => {
                    let status = self
                        .wait_for_natural_exit_after_eof()
                        .unwrap_or_else(|error| {
                            panic!(
                                "scenario `{scenario_id}` parent could not inspect worker after stdout EOF: {error}\n{}",
                                artifacts.summary()
                            )
                        });
                    self.stdin.take();
                    self.join_output_reader();
                    match status {
                        Some(status) => panic!(
                            "scenario `{scenario_id}` worker exited before publishing its pre-exit census: {}\n{}",
                            describe_exit_status(&status),
                            artifacts.summary()
                        ),
                        None => panic!(
                            "scenario `{scenario_id}` worker closed stdout before publishing its pre-exit census but remained alive for {WORKER_EXIT_AFTER_EOF_TIMEOUT:?}; the harness will terminate its job\n{}",
                            artifacts.summary()
                        ),
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    panic!(
                        "scenario `{scenario_id}` worker report channel closed without a report\n{}",
                        artifacts.summary()
                    )
                }
            }
            match self
                .child
                .as_mut()
                .expect("worker child must be live")
                .try_wait()
            {
                Ok(Some(status)) => {
                    self.stdin.take();
                    self.join_output_reader();
                    panic!(
                        "scenario `{scenario_id}` worker exited before publishing its pre-exit census: {}\n{}",
                        describe_exit_status(&status),
                        artifacts.summary()
                    )
                }
                Ok(None) => {}
                Err(error) => {
                    panic!(
                        "scenario `{scenario_id}` parent could not inspect worker while awaiting its census: {error}\n{}",
                        artifacts.summary()
                    )
                }
            }
            assert!(
                Instant::now() < deadline,
                "scenario `{scenario_id}` worker did not publish its pre-exit census within {WORKER_PROCESS_TIMEOUT:?}\n{}",
                artifacts.summary()
            );
        }
    }

    fn wait_for_natural_exit_after_eof(&mut self) -> std::io::Result<Option<ExitStatus>> {
        let deadline = Instant::now() + WORKER_EXIT_AFTER_EOF_TIMEOUT;
        loop {
            match self
                .child
                .as_mut()
                .expect("worker child must be live")
                .try_wait()?
            {
                Some(status) => return Ok(Some(status)),
                None if Instant::now() < deadline => thread::sleep(WORKER_POLL_INTERVAL),
                None => return Ok(None),
            }
        }
    }

    fn wait_for_exit(&mut self, scenario_id: &str, artifacts: &WorkerArtifacts) -> ExitStatus {
        let deadline = Instant::now() + WORKER_PROCESS_TIMEOUT;
        loop {
            match self
                .child
                .as_mut()
                .expect("worker child must be live")
                .try_wait()
            {
                Ok(Some(status)) => {
                    self.stdin.take();
                    self.join_output_reader();
                    return status;
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(WORKER_POLL_INTERVAL),
                Ok(None) => {
                    let termination = self.job.terminate();
                    let status = self
                        .child
                        .as_mut()
                        .expect("worker child must be live")
                        .wait();
                    panic!(
                        "scenario `{scenario_id}` worker job exceeded {WORKER_PROCESS_TIMEOUT:?}; termination={termination:?}; wait={status:?}\n{}",
                        artifacts.summary()
                    );
                }
                Err(error) => {
                    let _ = self.job.terminate();
                    let _ = self
                        .child
                        .as_mut()
                        .expect("worker child must be live")
                        .wait();
                    panic!(
                        "scenario `{scenario_id}` parent could not inspect worker status: {error}\n{}",
                        artifacts.summary()
                    );
                }
            }
        }
    }

    fn job_active_processes(&self) -> u32 {
        self.job
            .active_processes()
            .expect("native worker job census must remain readable")
    }

    fn join_output_reader(&mut self) {
        if let Some(reader) = self.output_reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for WorkerChild {
    fn drop(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        if child.try_wait().ok().flatten().is_none() {
            let _ = self.job.terminate();
            let _ = child.wait();
        }
        self.stdin.take();
        self.join_output_reader();
    }
}

fn spawn_worker_output_reader(
    stdout: ChildStdout,
    stdout_path: PathBuf,
    protocol_nonce: String,
    report: mpsc::Sender<WorkerOutputEvent>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("native-dock-worker-output".to_owned())
        .spawn(move || {
            let mut log = match File::create(&stdout_path) {
                Ok(log) => log,
                Err(error) => {
                    let _ = report.send(WorkerOutputEvent::ReadFailure(format!(
                        "could not create stdout log `{}`: {error}",
                        stdout_path.display()
                    )));
                    return;
                }
            };
            let prefix = format!("{WORKER_REPORT_PREFIX} {protocol_nonce} ");
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let mut report_sent = false;
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let _ = log.write_all(line.as_bytes());
                        let _ = log.flush();
                        if !report_sent && let Some(offset) = line.find(&prefix) {
                            let body = line[offset + prefix.len()..].trim_end_matches(['\r', '\n']);
                            let parsed = serde_json::from_str(body).map_err(|error| {
                                format!("invalid worker report: {error}; report={body}")
                            });
                            report_sent = true;
                            let _ = report.send(WorkerOutputEvent::Report(parsed));
                        }
                    }
                    Err(error) => {
                        if !report_sent {
                            let _ = report.send(WorkerOutputEvent::ReadFailure(format!(
                                "could not read worker stdout: {error}"
                            )));
                        }
                        return;
                    }
                }
            }
            if !report_sent {
                let _ = report.send(WorkerOutputEvent::Eof);
            }
        })
        .expect("native worker output reader must start")
}

struct WorkerJob(HANDLE);

impl WorkerJob {
    fn new() -> Result<Self> {
        unsafe {
            let job = Self(CreateJobObjectW(None, None).context("failed to create job object")?);
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .context("failed to set kill-on-close job limits")?;
            Ok(job)
        }
    }

    fn assign_process(&self, process_id: u32) -> Result<()> {
        unsafe {
            let process = OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                process_id,
            )
            .context("failed to open the native worker process")?;
            let result = AssignProcessToJobObject(self.0, process)
                .context("failed to assign the native worker process to its job");
            let _ = CloseHandle(process);
            result
        }
    }

    fn active_processes(&self) -> Result<u32> {
        unsafe {
            let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            QueryInformationJobObject(
                Some(self.0),
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                None,
            )
            .context("failed to query the native worker job census")?;
            Ok(info.ActiveProcesses)
        }
    }

    fn terminate(&self) -> Result<()> {
        unsafe { TerminateJobObject(self.0, 1).context("failed to terminate native worker job") }
    }
}

impl Drop for WorkerJob {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn describe_exit_status(status: &ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("{status} (code={code}, raw=0x{:08X})", code as u32),
        None => status.to_string(),
    }
}

fn read_log_tail(path: &Path) -> String {
    let Ok(mut file) = File::open(path) else {
        return "<unavailable>".to_owned();
    };
    let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
        return "<unavailable>".to_owned();
    };
    let tail_length = length.min(LOG_TAIL_BYTES as u64) as usize;
    if file.seek(SeekFrom::End(-(tail_length as i64))).is_err() {
        return "<unavailable>".to_owned();
    }
    let mut bytes = vec![0; tail_length];
    if file.read_exact(&mut bytes).is_err() {
        return "<unavailable>".to_owned();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
