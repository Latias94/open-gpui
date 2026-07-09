use std::{io, path::PathBuf, thread, time::Duration};

use clap::{Args, Subcommand, ValueEnum};
use open_gpui_devtools::{DevtoolsReport, DevtoolsReportSeverity};

use super::{
    artifact::{LoadedArtifact, WaitArgs, load_artifact, load_capture, load_report},
    query::{AssertArgs, QueryArgs, assert_command, query_command},
    render::{
        DevtoolsOutputFormat, DevtoolsStreamFormat, render_diff_markdown, render_report,
        write_output, write_stream_record,
    },
    watch::{FollowArgs, follow_command},
};

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
    /// Query typed rows from a DevTools artifact.
    Query(QueryArgs),
    /// Assert DevTools query, finding, generation, or diff conditions.
    Assert(AssertArgs),
    /// Follow a latest artifact file or appended JSONL artifact stream.
    Follow(FollowArgs),
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

pub(crate) fn devtools(_root: &std::path::Path, args: DevtoolsArgs) -> Result<(), ()> {
    match args.command {
        DevtoolsCommand::Report(args) => report(args),
        DevtoolsCommand::Diagnose(args) => diagnose(args),
        DevtoolsCommand::Diff(args) => diff(args),
        DevtoolsCommand::Stream(args) => stream(args),
        DevtoolsCommand::Query(args) => query_command(args),
        DevtoolsCommand::Assert(args) => assert_command(args),
        DevtoolsCommand::Follow(args) => follow_command(args),
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

fn sleep_between_records(interval: Duration) {
    if !interval.is_zero() {
        thread::sleep(interval);
    }
}

fn limit_or_all(limit: Option<usize>) -> usize {
    limit.unwrap_or(usize::MAX)
}
