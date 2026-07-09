use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use clap::{Args, Subcommand, ValueEnum};
use open_gpui_devtools::{
    DevtoolsCapture, DevtoolsCaptureDiff, DevtoolsReport, DevtoolsReportSeverity,
    DevtoolsSessionExport, DevtoolsSessionImportLimits,
};
use serde_json::json;

#[derive(Args, Debug)]
pub(crate) struct DevtoolsArgs {
    #[command(subcommand)]
    command: DevtoolsCommand,
}

#[derive(Subcommand, Debug)]
enum DevtoolsCommand {
    /// Build a machine-readable report from a DevTools capture or session export.
    Report(ReportArgs),
    /// Print diagnostics and optionally fail when findings cross a severity threshold.
    Diagnose(DiagnoseArgs),
    /// Diff two DevTools captures or current session-export frames.
    Diff(DiffArgs),
    /// Stream retained session frames as JSONL or markdown.
    Stream(StreamArgs),
}

#[derive(Args, Debug)]
struct ReportArgs {
    /// Input DevTools capture, session export, or report JSON.
    #[arg(short, long, value_name = "PATH")]
    input: PathBuf,
    /// Output path. Defaults to stdout.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Output format.
    #[arg(short, long, value_enum, default_value_t = DevtoolsOutputFormat::Json)]
    format: DevtoolsOutputFormat,
    #[command(flatten)]
    wait: WaitArgs,
}

#[derive(Args, Debug)]
struct DiagnoseArgs {
    /// Input DevTools capture, session export, or report JSON.
    #[arg(short, long, value_name = "PATH")]
    input: PathBuf,
    /// Output path. Defaults to stdout.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Output format.
    #[arg(short, long, value_enum, default_value_t = DevtoolsOutputFormat::Markdown)]
    format: DevtoolsOutputFormat,
    /// Return a failing exit status when a finding reaches this severity.
    #[arg(long, value_enum, default_value_t = FailOnSeverity::Error)]
    fail_on: FailOnSeverity,
    #[command(flatten)]
    wait: WaitArgs,
}

#[derive(Args, Debug)]
struct DiffArgs {
    /// Previous DevTools capture or session export.
    #[arg(long, value_name = "PATH")]
    before: PathBuf,
    /// Current DevTools capture or session export.
    #[arg(long, value_name = "PATH")]
    after: PathBuf,
    /// Output path. Defaults to stdout.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Output format.
    #[arg(short, long, value_enum, default_value_t = DevtoolsOutputFormat::Json)]
    format: DevtoolsOutputFormat,
    #[command(flatten)]
    wait: WaitArgs,
}

#[derive(Args, Debug)]
struct StreamArgs {
    /// Input DevTools session export, capture, or report JSON.
    #[arg(short, long, value_name = "PATH")]
    input: PathBuf,
    /// Stream format.
    #[arg(short, long, value_enum, default_value_t = DevtoolsStreamFormat::Jsonl)]
    format: DevtoolsStreamFormat,
    /// Maximum number of frames or records to emit.
    #[arg(long)]
    limit: Option<usize>,
    /// Delay between emitted records.
    #[arg(long, default_value_t = 0)]
    interval_ms: u64,
    #[command(flatten)]
    wait: WaitArgs,
}

#[derive(Args, Clone, Copy, Debug)]
struct WaitArgs {
    /// Milliseconds to wait for an artifact to appear or finish writing. Zero is fail-fast.
    #[arg(long, default_value_t = 0)]
    timeout_ms: u64,
    /// Poll interval used when --timeout-ms is non-zero.
    #[arg(long, default_value_t = 100)]
    poll_ms: u64,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DevtoolsOutputFormat {
    Json,
    Markdown,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DevtoolsStreamFormat {
    Jsonl,
    Markdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum FailOnSeverity {
    None,
    Info,
    Warning,
    Error,
}

impl FailOnSeverity {
    fn threshold(self) -> Option<DevtoolsReportSeverity> {
        match self {
            Self::None => None,
            Self::Info => Some(DevtoolsReportSeverity::Info),
            Self::Warning => Some(DevtoolsReportSeverity::Warning),
            Self::Error => Some(DevtoolsReportSeverity::Error),
        }
    }
}

enum LoadedArtifact {
    Capture(DevtoolsCapture),
    SessionExport(DevtoolsSessionExport),
    Report(DevtoolsReport),
}

pub(crate) fn devtools(_root: &Path, args: DevtoolsArgs) -> Result<(), ()> {
    match args.command {
        DevtoolsCommand::Report(args) => report(args),
        DevtoolsCommand::Diagnose(args) => diagnose(args),
        DevtoolsCommand::Diff(args) => diff(args),
        DevtoolsCommand::Stream(args) => stream(args),
    }
}

fn report(args: ReportArgs) -> Result<(), ()> {
    let report = load_report(&args.input, args.wait)?;
    write_output(args.output.as_deref(), render_report(&report, args.format))
}

fn diagnose(args: DiagnoseArgs) -> Result<(), ()> {
    let report = load_report(&args.input, args.wait)?;
    write_output(args.output.as_deref(), render_report(&report, args.format))?;

    if let Some(threshold) = args.fail_on.threshold() {
        if report.has_finding_at_or_above(threshold) {
            eprintln!(
                "devtools diagnostics reached --fail-on threshold `{}`",
                threshold.as_label()
            );
            return Err(());
        }
    }

    Ok(())
}

fn diff(args: DiffArgs) -> Result<(), ()> {
    let before = load_capture(&args.before, args.wait)?;
    let after = load_capture(&args.after, args.wait)?;
    let diff = after.diff_from(&before);
    let rendered = match args.format {
        DevtoolsOutputFormat::Json => serde_json::to_string_pretty(&diff).map_err(|error| {
            eprintln!("failed to serialize devtools diff: {error}");
        })?,
        DevtoolsOutputFormat::Markdown => render_diff_markdown(&diff),
    };
    write_output(args.output.as_deref(), Ok(rendered))
}

fn stream(args: StreamArgs) -> Result<(), ()> {
    let artifact = load_artifact(&args.input, args.wait)?;
    let interval = Duration::from_millis(args.interval_ms);
    let mut stdout = io::stdout().lock();

    match artifact {
        LoadedArtifact::SessionExport(export) => {
            for (index, frame) in export
                .frames
                .iter()
                .enumerate()
                .take(limit_or_all(args.limit))
            {
                let report = DevtoolsReport::from_session_frame(frame);
                write_stream_record(&mut stdout, args.format, index, &report)?;
                sleep_between_records(interval);
            }
        }
        LoadedArtifact::Capture(capture) => {
            let report = DevtoolsReport::from_capture(&capture);
            write_stream_record(&mut stdout, args.format, 0, &report)?;
        }
        LoadedArtifact::Report(report) => {
            write_stream_record(&mut stdout, args.format, 0, &report)?;
        }
    }

    Ok(())
}

fn load_report(path: &Path, wait: WaitArgs) -> Result<DevtoolsReport, ()> {
    match load_artifact(path, wait)? {
        LoadedArtifact::Capture(capture) => Ok(DevtoolsReport::from_capture(&capture)),
        LoadedArtifact::SessionExport(export) => Ok(DevtoolsReport::from_session_export(&export)),
        LoadedArtifact::Report(report) => Ok(report),
    }
}

fn load_capture(path: &Path, wait: WaitArgs) -> Result<DevtoolsCapture, ()> {
    match load_artifact(path, wait)? {
        LoadedArtifact::Capture(capture) => Ok(capture),
        LoadedArtifact::SessionExport(export) => export
            .frames
            .last()
            .map(|frame| frame.capture.clone())
            .ok_or_else(|| {
                eprintln!(
                    "devtools session export `{}` has no current frame",
                    path.display()
                );
            }),
        LoadedArtifact::Report(_) => {
            eprintln!(
                "devtools report `{}` does not contain raw capture data for diffing",
                path.display()
            );
            Err(())
        }
    }
}

fn load_artifact(path: &Path, wait: WaitArgs) -> Result<LoadedArtifact, ()> {
    let timeout = Duration::from_millis(wait.timeout_ms);
    let poll = Duration::from_millis(wait.poll_ms.max(1));
    let started = Instant::now();

    loop {
        match load_artifact_once(path) {
            Ok(artifact) => return Ok(artifact),
            Err(error) => {
                if timeout.is_zero() || started.elapsed() >= timeout {
                    eprintln!("{error}");
                    return Err(());
                }
                thread::sleep(poll);
            }
        }
    }
}

fn load_artifact_once(path: &Path) -> Result<LoadedArtifact, String> {
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read devtools artifact `{}`: {error}",
            path.display()
        )
    })?;
    if let Ok(report) = serde_json::from_str::<DevtoolsReport>(&source) {
        return Ok(LoadedArtifact::Report(report));
    }

    if let Ok(export) =
        DevtoolsSessionExport::from_json_str(&source, DevtoolsSessionImportLimits::default())
    {
        return Ok(LoadedArtifact::SessionExport(export));
    }

    if let Ok(capture) = serde_json::from_str::<DevtoolsCapture>(&source) {
        return Ok(LoadedArtifact::Capture(capture.sanitized()));
    }

    Err(format!(
        "unsupported devtools artifact `{}`: expected report, session export, or capture JSON",
        path.display()
    ))
}

fn render_report(report: &DevtoolsReport, format: DevtoolsOutputFormat) -> Result<String, ()> {
    match format {
        DevtoolsOutputFormat::Json => serde_json::to_string_pretty(report).map_err(|error| {
            eprintln!("failed to serialize devtools report: {error}");
        }),
        DevtoolsOutputFormat::Markdown => Ok(report.to_markdown()),
    }
}

fn render_diff_markdown(diff: &DevtoolsCaptureDiff) -> String {
    let mut output = String::from("# Open GPUI DevTools Diff\n\n");
    output.push_str("| Status | Count |\n|---|---:|\n");
    output.push_str(&format!("| added | {} |\n", diff.summary.added));
    output.push_str(&format!("| removed | {} |\n", diff.summary.removed));
    output.push_str(&format!("| changed | {} |\n", diff.summary.changed));
    output.push_str(&format!("| unchanged | {} |\n", diff.summary.unchanged));
    output.push_str(&format!("| collisions | {} |\n", diff.summary.collisions));
    output.push_str("\n## Rows\n\n");
    output.push_str("| Kind | Status | Identity | Label |\n|---|---|---|---|\n");
    for row in &diff.rows {
        output.push_str(&format!(
            "| {} | {} | `{}` | {} |\n",
            row.kind.as_label(),
            row.status.as_label(),
            row.identity,
            row.label.replace('|', "\\|")
        ));
    }
    output
}

fn write_stream_record(
    stdout: &mut io::StdoutLock<'_>,
    format: DevtoolsStreamFormat,
    sequence: usize,
    report: &DevtoolsReport,
) -> Result<(), ()> {
    match format {
        DevtoolsStreamFormat::Jsonl => {
            let record = json!({
                "schema_version": "open-gpui-devtools-stream/v1",
                "sequence": sequence,
                "kind": "report",
                "report": report,
            });
            writeln!(stdout, "{record}").map_err(|error| {
                eprintln!("failed to write devtools stream record: {error}");
            })?;
        }
        DevtoolsStreamFormat::Markdown => {
            writeln!(stdout, "<!-- devtools-stream sequence={sequence} -->").map_err(|error| {
                eprintln!("failed to write devtools stream marker: {error}");
            })?;
            writeln!(stdout, "{}", report.to_markdown()).map_err(|error| {
                eprintln!("failed to write devtools stream report: {error}");
            })?;
        }
    }
    stdout.flush().map_err(|error| {
        eprintln!("failed to flush devtools stream record: {error}");
    })
}

fn write_output(path: Option<&Path>, output: Result<String, ()>) -> Result<(), ()> {
    let output = output?;
    match path {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    eprintln!(
                        "failed to create output directory `{}`: {error}",
                        parent.display()
                    );
                })?;
            }
            fs::write(path, output).map_err(|error| {
                eprintln!("failed to write `{}`: {error}", path.display());
            })
        }
        None => {
            println!("{output}");
            Ok(())
        }
    }
}

fn sleep_between_records(interval: Duration) {
    if !interval.is_zero() {
        thread::sleep(interval);
    }
}

fn limit_or_all(limit: Option<usize>) -> usize {
    limit.unwrap_or(usize::MAX)
}
