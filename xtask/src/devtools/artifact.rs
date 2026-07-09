use std::{
    fs,
    io::{self, Read},
    path::Path,
    thread,
    time::{Duration, Instant},
};

use clap::Args;
use open_gpui_devtools::{
    DevtoolsArtifact, DevtoolsArtifactRecord, DevtoolsCapture, DevtoolsReport,
    DevtoolsSessionExport, DevtoolsSessionImportLimits,
};

#[derive(Args, Clone, Copy, Debug)]
pub(super) struct WaitArgs {
    /// Milliseconds to wait for an artifact to appear or finish writing. Zero is fail-fast.
    #[arg(long, default_value_t = 0)]
    pub(super) timeout_ms: u64,
    /// Poll interval used when --timeout-ms is non-zero.
    #[arg(long, default_value_t = 100)]
    pub(super) poll_ms: u64,
}

pub(super) enum LoadedArtifact {
    Capture(DevtoolsCapture),
    SessionExport(DevtoolsSessionExport),
    Report(DevtoolsReport),
}

pub(super) struct LoadedArtifactSnapshot {
    pub(super) artifact: LoadedArtifact,
    pub(super) source: String,
}

pub(super) fn load_report(path: &Path, wait: WaitArgs) -> Result<DevtoolsReport, ()> {
    match load_artifact(path, wait)? {
        LoadedArtifact::Capture(capture) => Ok(DevtoolsReport::from_capture(&capture)),
        LoadedArtifact::SessionExport(export) => Ok(DevtoolsReport::from_session_export(&export)),
        LoadedArtifact::Report(report) => Ok(report),
    }
}

pub(super) fn load_capture(path: &Path, wait: WaitArgs) -> Result<DevtoolsCapture, ()> {
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

pub(super) fn load_artifact(path: &Path, wait: WaitArgs) -> Result<LoadedArtifact, ()> {
    load_artifact_snapshot(path, wait).map(|snapshot| snapshot.artifact)
}

pub(super) fn load_artifact_snapshot(
    path: &Path,
    wait: WaitArgs,
) -> Result<LoadedArtifactSnapshot, ()> {
    let timeout = Duration::from_millis(wait.timeout_ms);
    let poll = Duration::from_millis(wait.poll_ms.max(1));
    let started = Instant::now();

    loop {
        match try_load_artifact_snapshot_once(path) {
            Ok(snapshot) => return Ok(snapshot),
            Err(error) => {
                if path_is_stdin(path) || timeout.is_zero() || started.elapsed() >= timeout {
                    eprintln!("{error}");
                    return Err(());
                }
                thread::sleep(poll);
            }
        }
    }
}

pub(super) fn try_load_artifact_snapshot_once(
    path: &Path,
) -> Result<LoadedArtifactSnapshot, String> {
    let source = read_artifact_source_once(path)?;
    let artifact = parse_artifact_source(&source, &artifact_source_label(path))?;
    Ok(LoadedArtifactSnapshot { artifact, source })
}

pub(super) fn parse_artifact_source(
    source: &str,
    source_label: &str,
) -> Result<LoadedArtifact, String> {
    if let Ok(record) = serde_json::from_str::<DevtoolsArtifactRecord>(source) {
        return Ok(match record.artifact {
            DevtoolsArtifact::Capture(capture) => LoadedArtifact::Capture(capture.sanitized()),
            DevtoolsArtifact::SessionExport(export) => LoadedArtifact::SessionExport(export),
            DevtoolsArtifact::Report(report) => LoadedArtifact::Report(report),
        });
    }

    if let Ok(report) = serde_json::from_str::<DevtoolsReport>(source) {
        return Ok(LoadedArtifact::Report(report));
    }

    if let Ok(export) =
        DevtoolsSessionExport::from_json_str(source, DevtoolsSessionImportLimits::default())
    {
        return Ok(LoadedArtifact::SessionExport(export));
    }

    if let Ok(capture) = serde_json::from_str::<DevtoolsCapture>(source) {
        return Ok(LoadedArtifact::Capture(capture.sanitized()));
    }

    Err(format!(
        "unsupported devtools artifact `{source_label}`: expected artifact record, report, session export, or capture JSON"
    ))
}

fn read_artifact_source_once(path: &Path) -> Result<String, String> {
    if path_is_stdin(path) {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| format!("failed to read devtools artifact from stdin: {error}"))?;
        return Ok(source);
    }

    fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read devtools artifact `{}`: {error}",
            path.display()
        )
    })
}

pub(super) fn path_is_stdin(path: &Path) -> bool {
    path == Path::new("-")
}

fn artifact_source_label(path: &Path) -> String {
    if path_is_stdin(path) {
        "stdin".to_owned()
    } else {
        path.display().to_string()
    }
}
