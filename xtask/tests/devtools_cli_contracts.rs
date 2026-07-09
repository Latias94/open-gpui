use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

fn xtask_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_xtask"))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a workspace parent")
        .to_path_buf()
}

fn fixture(name: &str) -> PathBuf {
    workspace_root()
        .join("crates")
        .join("devtools")
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn query_reads_capture_from_stdin() {
    let capture = fs::read_to_string(fixture("simple-capture.json")).unwrap();
    let output = run_xtask_with_stdin(
        [
            "devtools",
            "query",
            "--input",
            "-",
            "--target-kind",
            "viewport",
        ],
        &capture,
    );

    assert_success(&output);
    let json = stdout_json(&output);
    assert_eq!(json["row_count"], 1);
    assert_eq!(json["rows"][0]["kind"], "target");
    assert_eq!(json["rows"][0]["details"]["target_kind"], "viewport");
}

#[test]
fn query_selectors_return_deterministic_rows() {
    let capture = fixture("simple-capture.json");
    assert_query_count(&capture, ["--domain-kind", "docking"], 1);
    assert_query_count(&capture, ["--event-id", "viewport.opened"], 1);
    assert_query_count(&capture, ["--snapshot-kind", "layout"], 1);
    assert_query_count(&capture, ["--finding-at-or-above", "warning"], 1);

    let session = fixture("simple-session.json");
    assert_query_count(&session, ["--generation", "2"], 1);
    assert_query_count(&session, ["--diff-status", "changed"], 2);
}

#[test]
fn assert_missing_selector_fails_with_machine_readable_reason() {
    let output = run_xtask([
        "devtools",
        "assert",
        "--input",
        fixture("simple-capture.json").to_str().unwrap(),
        "--target-kind",
        "missing",
        "--format",
        "json",
    ]);

    assert_failure(&output);
    let json = stdout_json(&output);
    assert_eq!(json["ok"], false);
    assert_eq!(
        json["failures"][0]["code"],
        "devtools.assert.no-query-match"
    );
}

#[test]
fn assert_fail_on_finding_threshold_is_a_health_gate() {
    let warning_output = run_xtask([
        "devtools",
        "assert",
        "--input",
        fixture("simple-capture.json").to_str().unwrap(),
        "--fail-on-finding",
        "warning",
    ]);
    assert_failure(&warning_output);
    assert_eq!(
        stdout_json(&warning_output)["failures"][0]["code"],
        "devtools.assert.finding-threshold"
    );

    let clean_output = run_xtask([
        "devtools",
        "assert",
        "--input",
        fixture("simple-session.json").to_str().unwrap(),
        "--fail-on-finding",
        "warning",
    ]);
    assert_success(&clean_output);
    assert_eq!(stdout_json(&clean_output)["ok"], true);
}

#[test]
fn query_waits_for_artifact_until_timeout() {
    let temp = TestDir::new("wait");
    let artifact = temp.path.join("latest.json");

    let fail_fast = run_xtask([
        "devtools",
        "query",
        "--input",
        artifact.to_str().unwrap(),
        "--target-kind",
        "viewport",
    ]);
    assert_failure(&fail_fast);

    let child = Command::new(xtask_bin())
        .args([
            "devtools",
            "query",
            "--input",
            artifact.to_str().unwrap(),
            "--target-kind",
            "viewport",
            "--timeout-ms",
            "2000",
            "--poll-ms",
            "25",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    thread::sleep(Duration::from_millis(100));
    fs::copy(fixture("simple-capture.json"), &artifact).unwrap();

    let output = child.wait_with_output().unwrap();
    assert_success(&output);
    assert_eq!(stdout_json(&output)["row_count"], 1);
}

#[test]
fn follow_latest_emits_new_generation_records() {
    let temp = TestDir::new("follow-latest");
    let latest = temp.path.join("latest.json");

    let child = Command::new(xtask_bin())
        .args([
            "devtools",
            "follow",
            "--input",
            latest.to_str().unwrap(),
            "--limit",
            "2",
            "--timeout-ms",
            "2000",
            "--poll-ms",
            "25",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    thread::sleep(Duration::from_millis(100));
    fs::write(&latest, session_with_generation(1)).unwrap();
    thread::sleep(Duration::from_millis(100));
    fs::write(&latest, session_with_generation(2)).unwrap();

    let output = child.wait_with_output().unwrap();
    assert_success(&output);
    let records = stdout_json_lines(&output);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["generation"], 1);
    assert_eq!(records[1]["generation"], 2);
}

#[test]
fn follow_jsonl_consumes_appended_artifact_records() {
    let temp = TestDir::new("follow-jsonl");
    let jsonl = temp.path.join("artifacts.jsonl");
    let capture = minified_fixture("simple-capture.json");
    fs::write(&jsonl, format!("{capture}\n{capture}\n")).unwrap();

    let output = run_xtask([
        "devtools",
        "follow",
        "--input",
        jsonl.to_str().unwrap(),
        "--input-mode",
        "jsonl",
        "--target-kind",
        "viewport",
        "--limit",
        "2",
    ]);

    assert_success(&output);
    let records = stdout_json_lines(&output);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["kind"], "query");
    assert_eq!(records[0]["query"]["row_count"], 1);
}

#[test]
fn follow_jsonl_waits_for_complete_appended_line() {
    let temp = TestDir::new("follow-jsonl-partial");
    let jsonl = temp.path.join("artifacts.jsonl");
    let capture = minified_fixture("simple-capture.json");
    let split = capture.len() / 2;
    fs::write(&jsonl, &capture[..split]).unwrap();

    let child = Command::new(xtask_bin())
        .args([
            "devtools",
            "follow",
            "--input",
            jsonl.to_str().unwrap(),
            "--input-mode",
            "jsonl",
            "--target-kind",
            "viewport",
            "--limit",
            "1",
            "--timeout-ms",
            "2000",
            "--poll-ms",
            "25",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    thread::sleep(Duration::from_millis(100));
    let mut file = fs::OpenOptions::new().append(true).open(&jsonl).unwrap();
    writeln!(file, "{}", &capture[split..]).unwrap();

    let output = child.wait_with_output().unwrap();
    assert_success(&output);
    let records = stdout_json_lines(&output);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["kind"], "query");
    assert_eq!(records[0]["query"]["row_count"], 1);
}

fn assert_query_count<const N: usize>(input: &Path, selector_args: [&str; N], expected: usize) {
    let mut args = vec![
        "devtools".to_owned(),
        "query".to_owned(),
        "--input".to_owned(),
        input.to_str().unwrap().to_owned(),
    ];
    args.extend(selector_args.into_iter().map(str::to_owned));
    let output = Command::new(xtask_bin()).args(args).output().unwrap();
    assert_success(&output);
    assert_eq!(stdout_json(&output)["row_count"], expected);
}

fn run_xtask<const N: usize>(args: [&str; N]) -> Output {
    Command::new(xtask_bin()).args(args).output().unwrap()
}

fn run_xtask_with_stdin<const N: usize>(args: [&str; N], stdin: &str) -> Output {
    let mut child = Command::new(xtask_bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout was not JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn stdout_json_lines(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn minified_fixture(name: &str) -> String {
    let value: Value = serde_json::from_str(&fs::read_to_string(fixture(name)).unwrap()).unwrap();
    serde_json::to_string(&value).unwrap()
}

fn session_with_generation(generation: u64) -> String {
    let mut session: Value =
        serde_json::from_str(&fs::read_to_string(fixture("simple-session.json")).unwrap()).unwrap();
    session["current_generation"] = Value::from(generation);
    let frames = session["frames"].as_array_mut().unwrap();
    frames.truncate(generation as usize);
    for (index, frame) in frames.iter_mut().enumerate() {
        let generation = (index + 1) as u64;
        frame["generation"] = Value::from(generation);
        frame["previous_generation"] = if generation == 1 {
            Value::Null
        } else {
            Value::from(generation - 1)
        };
    }
    session["retained_frames"] = Value::from(frames.len());
    serde_json::to_string_pretty(&session).unwrap()
}

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(label: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "open-gpui-devtools-cli-{label}-{}-{now}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let temp_root = env::temp_dir().canonicalize().unwrap();
        if let Ok(path) = self.path.canonicalize() {
            let name_ok = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("open-gpui-devtools-cli-"));
            if path.starts_with(&temp_root) && name_ok {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
}
