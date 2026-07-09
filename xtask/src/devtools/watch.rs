//! DevTools follow command implementation.

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use clap::{Args, ValueEnum};
use open_gpui_devtools::DevtoolsReport;
use serde_json::json;

use super::{
    artifact::{
        LoadedArtifact, WaitArgs, parse_artifact_source, path_is_stdin,
        try_load_artifact_snapshot_once,
    },
    query::{QueryOutputFormat, QuerySelectorArgs, query_artifact, render_query_result},
};

const FOLLOW_SCHEMA_VERSION: &str = "open-gpui-devtools-follow/v1";

#[derive(Args, Debug)]
pub(super) struct FollowArgs {
    /// Input latest artifact file, artifact JSONL file, or '-' for stdin JSONL.
    #[arg(short, long, value_name = "PATH")]
    input: PathBuf,
    /// Input mode. Defaults to jsonl for .jsonl files and latest otherwise.
    #[arg(long, value_enum)]
    input_mode: Option<FollowInputMode>,
    /// Follow output format.
    #[arg(short, long, value_enum, default_value_t = FollowOutputFormat::Jsonl)]
    format: FollowOutputFormat,
    /// Maximum emitted records.
    #[arg(long)]
    limit: Option<usize>,
    /// Stop after this many idle milliseconds once at least one record has been emitted.
    #[arg(long)]
    idle_after_ms: Option<u64>,
    #[command(flatten)]
    selectors: QuerySelectorArgs,
    #[command(flatten)]
    wait: WaitArgs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum FollowInputMode {
    Latest,
    Jsonl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum FollowOutputFormat {
    Jsonl,
    Markdown,
}

pub(super) fn follow_command(args: FollowArgs) -> Result<(), ()> {
    if args.limit == Some(0) {
        return Ok(());
    }
    let mode = args
        .input_mode
        .unwrap_or_else(|| infer_input_mode(&args.input));
    let mut stdout = io::stdout().lock();
    match mode {
        FollowInputMode::Latest => follow_latest(args, &mut stdout),
        FollowInputMode::Jsonl => follow_jsonl(args, &mut stdout),
    }
}

fn follow_latest(args: FollowArgs, stdout: &mut impl Write) -> Result<(), ()> {
    if path_is_stdin(&args.input) {
        eprintln!("devtools follow latest mode requires a file path, not stdin");
        return Err(());
    }

    let poll = Duration::from_millis(args.wait.poll_ms.max(1));
    let startup_timeout = Duration::from_millis(args.wait.timeout_ms);
    let idle_after = args.idle_after_ms.map(Duration::from_millis);
    let started = Instant::now();
    let mut last_emit = Instant::now();
    let mut emitted = 0usize;
    let mut last_identity: Option<FollowIdentity> = None;
    let mut last_error: Option<String> = None;

    loop {
        match try_load_artifact_snapshot_once(&args.input) {
            Ok(snapshot) => {
                let identity = FollowIdentity::new(&snapshot.artifact, &snapshot.source);
                if last_identity.as_ref() != Some(&identity) {
                    emit_follow_record(stdout, emitted, &snapshot.artifact, &args)?;
                    last_identity = Some(identity);
                    emitted += 1;
                    last_emit = Instant::now();
                    last_error = None;
                    if limit_reached(emitted, args.limit) {
                        return Ok(());
                    }
                }
            }
            Err(error) => {
                last_error = Some(error);
            }
        }

        if emitted == 0 && timeout_elapsed(started, startup_timeout) {
            if let Some(error) = last_error {
                eprintln!("{error}");
            } else {
                eprintln!(
                    "timed out waiting for devtools artifact `{}`",
                    args.input.display()
                );
            }
            return Err(());
        }
        if emitted > 0 && idle_elapsed(last_emit, idle_after) {
            return Ok(());
        }

        thread::sleep(poll);
    }
}

fn follow_jsonl(args: FollowArgs, stdout: &mut impl Write) -> Result<(), ()> {
    let poll = Duration::from_millis(args.wait.poll_ms.max(1));
    let startup_timeout = Duration::from_millis(args.wait.timeout_ms);
    let idle_after = args.idle_after_ms.map(Duration::from_millis);
    let started = Instant::now();
    let mut last_emit = Instant::now();
    let mut emitted = 0usize;
    let mut consumed_lines = 0usize;
    let mut last_error: Option<String> = None;

    if path_is_stdin(&args.input) {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source).map_err(|error| {
            eprintln!("failed to read devtools JSONL from stdin: {error}");
        })?;
        return emit_jsonl_lines(stdout, &source, consumed_lines, emitted, true, &args)
            .map(|_| ())
            .map_err(|error| {
                eprintln!("{error}");
            });
    }

    loop {
        match fs::read_to_string(&args.input) {
            Ok(source) => {
                match emit_jsonl_lines(stdout, &source, consumed_lines, emitted, false, &args) {
                    Ok(progress) => {
                        consumed_lines = progress.consumed_lines;
                        if progress.emitted > emitted {
                            emitted = progress.emitted;
                            last_emit = Instant::now();
                            last_error = None;
                            if limit_reached(emitted, args.limit) {
                                return Ok(());
                            }
                        }
                    }
                    Err(error) => {
                        last_error = Some(error);
                    }
                }
            }
            Err(error) => {
                last_error = Some(format!(
                    "failed to read devtools JSONL `{}`: {error}",
                    args.input.display()
                ));
            }
        }

        if emitted == 0 && timeout_elapsed(started, startup_timeout) {
            if let Some(error) = last_error {
                eprintln!("{error}");
            } else {
                eprintln!(
                    "timed out waiting for devtools JSONL `{}`",
                    args.input.display()
                );
            }
            return Err(());
        }
        if emitted > 0 && idle_elapsed(last_emit, idle_after) {
            return Ok(());
        }

        thread::sleep(poll);
    }
}

struct JsonlProgress {
    consumed_lines: usize,
    emitted: usize,
}

fn emit_jsonl_lines(
    stdout: &mut impl Write,
    source: &str,
    consumed_lines: usize,
    emitted: usize,
    allow_incomplete_final_line: bool,
    args: &FollowArgs,
) -> Result<JsonlProgress, String> {
    let mut consumed_lines = consumed_lines;
    let mut emitted = emitted;
    let lines = source.lines().collect::<Vec<_>>();
    let final_line_is_complete = source.ends_with('\n') || source.ends_with('\r');

    for (index, line) in lines.iter().enumerate().skip(consumed_lines) {
        if !allow_incomplete_final_line && !final_line_is_complete && index + 1 == lines.len() {
            break;
        }
        if line.trim().is_empty() {
            consumed_lines = index + 1;
            continue;
        }

        let artifact =
            parse_artifact_source(line, &format!("{}:{}", args.input.display(), index + 1))?;
        emit_follow_record(stdout, emitted, &artifact, args)
            .map_err(|()| "failed to write devtools follow record".to_owned())?;
        emitted += 1;
        consumed_lines = index + 1;
        if limit_reached(emitted, args.limit) {
            break;
        }
    }

    Ok(JsonlProgress {
        consumed_lines,
        emitted,
    })
}

fn emit_follow_record(
    stdout: &mut impl Write,
    sequence: usize,
    artifact: &LoadedArtifact,
    args: &FollowArgs,
) -> Result<(), ()> {
    let generation = artifact_generation(artifact);
    match args.format {
        FollowOutputFormat::Jsonl => {
            let record = if args.selectors.has_presence_selector() {
                let query = query_artifact(artifact, &args.selectors);
                json!({
                    "schema_version": FOLLOW_SCHEMA_VERSION,
                    "sequence": sequence,
                    "kind": "query",
                    "generation": generation,
                    "query": query,
                })
            } else {
                let report = report_for_artifact(artifact);
                json!({
                    "schema_version": FOLLOW_SCHEMA_VERSION,
                    "sequence": sequence,
                    "kind": "report",
                    "generation": generation,
                    "report": report,
                })
            };
            writeln!(stdout, "{record}").map_err(|error| {
                eprintln!("failed to write devtools follow JSONL record: {error}");
            })?;
        }
        FollowOutputFormat::Markdown => {
            writeln!(
                stdout,
                "<!-- devtools-follow sequence={sequence} generation={} -->",
                generation
                    .map(|generation| generation.to_string())
                    .unwrap_or_else(|| "none".to_owned())
            )
            .map_err(|error| {
                eprintln!("failed to write devtools follow markdown marker: {error}");
            })?;
            if args.selectors.has_presence_selector() {
                let query = query_artifact(artifact, &args.selectors);
                writeln!(
                    stdout,
                    "{}",
                    render_query_result(&query, QueryOutputFormat::Markdown)?
                )
                .map_err(|error| {
                    eprintln!("failed to write devtools follow query markdown: {error}");
                })?;
            } else {
                writeln!(stdout, "{}", report_for_artifact(artifact).to_markdown()).map_err(
                    |error| {
                        eprintln!("failed to write devtools follow report markdown: {error}");
                    },
                )?;
            }
        }
    }
    stdout.flush().map_err(|error| {
        eprintln!("failed to flush devtools follow record: {error}");
    })
}

#[derive(Eq, PartialEq)]
struct FollowIdentity {
    generation: Option<u64>,
    fingerprint: u64,
}

impl FollowIdentity {
    fn new(artifact: &LoadedArtifact, source: &str) -> Self {
        Self {
            generation: artifact_generation(artifact),
            fingerprint: fingerprint(source),
        }
    }
}

fn report_for_artifact(artifact: &LoadedArtifact) -> DevtoolsReport {
    match artifact {
        LoadedArtifact::Capture(capture) => DevtoolsReport::from_capture(capture),
        LoadedArtifact::SessionExport(export) => DevtoolsReport::from_session_export(export),
        LoadedArtifact::Report(report) => report.clone(),
    }
}

fn artifact_generation(artifact: &LoadedArtifact) -> Option<u64> {
    match artifact {
        LoadedArtifact::Capture(_) => None,
        LoadedArtifact::SessionExport(export) => export.current_generation,
        LoadedArtifact::Report(report) => report.source.generation,
    }
}

fn fingerprint(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

fn limit_reached(emitted: usize, limit: Option<usize>) -> bool {
    limit.is_some_and(|limit| emitted >= limit)
}

fn timeout_elapsed(started: Instant, timeout: Duration) -> bool {
    timeout.is_zero() || started.elapsed() >= timeout
}

fn idle_elapsed(last_emit: Instant, idle_after: Option<Duration>) -> bool {
    idle_after.is_some_and(|idle_after| last_emit.elapsed() >= idle_after)
}

fn infer_input_mode(path: &Path) -> FollowInputMode {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
    {
        FollowInputMode::Jsonl
    } else {
        FollowInputMode::Latest
    }
}
